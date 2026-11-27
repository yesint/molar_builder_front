import { builderApi } from "./builder-api.js";

const api = builderApi + "/builder";
let currentKey = null;
let currentPlan = null;
const popupOrderKey = "molar-paypal-return-order";

function openPaymentWindow() {
  const popup = window.open("about:blank", "_blank", "popup,width=600,height=750");
  if (!popup) throw new Error("Allow popups to open payment.");
  popup.opener = null;
  return popup;
}

export class CheckoutDismissed extends Error {}

export function buildHeaders(headers = {}) {
  return currentKey ? { ...headers, "X-Builder-Job": currentKey } : headers;
}

export async function sha256(data) {
  const bytes = data instanceof Blob ? await data.arrayBuffer() : new TextEncoder().encode(data);
  return Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
    n => n.toString(16).padStart(2, "0")).join("");
}

async function paymentRequest(path, body, signal) {
  const response = await fetch(api + path, {
    method: "POST", headers: buildHeaders({"Content-Type": "application/json"}),
    body: JSON.stringify(body), signal: AbortSignal.any([signal, AbortSignal.timeout(60000)]),
  });
  const result = await response.json();
  if (!response.ok) throw new Error(result.error || "PayPal checkout could not be confirmed.");
  return result;
}

export function receiveCheckoutReturn(notice) {
  const params = new URLSearchParams(location.hash.slice(1));
  const outcome = params.get("checkout");
  if (outcome !== "return" && outcome !== "cancel") return;
  history.replaceState(null, "", location.pathname + location.search);
  notice.textContent = outcome === "return"
    ? "You returned from PayPal. The builder tab where you submitted the job shows its progress."
    : "You cancelled checkout. You can resume it in the builder tab where you submitted the job.";
  notice.hidden = false;
  const order = params.get("order") || sessionStorage.getItem(popupOrderKey) || "";
  sessionStorage.removeItem(popupOrderKey);
  if (!/^[A-Z0-9]{8,32}$/.test(order)) return;
  const channel = new BroadcastChannel("molar-paypal-" + order);
  channel.onmessage = ({data}) => {
    if (data === "received") {
      channel.close();
      window.close();
    }
  };
  channel.postMessage(outcome);
  setTimeout(() => channel.close(), 10000);
}

export async function checkoutBuild(makePlan, mode = "paypal-sandbox", {signal, onConfirming} = {}) {
  // The popup must open during the original Submit click.
  const popup = openPaymentWindow();
  let channel, wake;
  let returned = false;
  const wait = new AbortController();
  function backToBuilder() {
    returned = true;
    wake?.();
  }
  function stopWaiting() {
    wait.abort(new CheckoutDismissed("Checkout closed. Submit again to resume with the same build settings."));
    popup.close();
    wake?.();
  }
  signal?.addEventListener("abort", stopWaiting, {once: true});
  try {
    if (signal?.aborted) stopWaiting();
    wait.signal.throwIfAborted();
    const plan = await makePlan();
    currentPlan = (mode === "paypal-live" ? "molar-live:" : "molar-sandbox:") + await sha256(JSON.stringify(plan));
    currentKey = sessionStorage.getItem(currentPlan);
    if (!currentKey) {
      currentKey = Array.from(crypto.getRandomValues(new Uint8Array(32)), n => n.toString(16).padStart(2, "0")).join("");
      sessionStorage.setItem(currentPlan, currentKey);
    }
    const order = await paymentRequest("/checkout", plan, wait.signal);
    wait.signal.throwIfAborted();
    if (order.paid) return;
    const url = new URL(order.url);
    const paypalOrigin = mode === "paypal-live" ? "https://www.paypal.com" : "https://www.sandbox.paypal.com";
    if (url.origin !== paypalOrigin) throw new Error("Unexpected PayPal checkout address.");
    const orderId = order.order_id || url.searchParams.get("token");
    if (!/^[A-Z0-9]{8,32}$/.test(orderId || "")) throw new Error("Missing PayPal order ID.");
    channel = new BroadcastChannel("molar-paypal-" + orderId);
    channel.onmessage = ({data}) => {
      if (data !== "return" && data !== "cancel") return;
      channel.postMessage("received");
      if (data === "cancel") stopWaiting();
      else {
        backToBuilder();
        popup.close();
      }
      window.focus();
    };
    // An error return from PayPal can omit the order token. The popup keeps
    // its own session storage when it returns to the builder origin.
    if (!popup.closed && !signal?.aborted) {
      popup.sessionStorage?.setItem(popupOrderKey, orderId);
      popup.location.replace(url.href);
    }
    window.addEventListener("focus", backToBuilder, {once: true});
    const deadline = Date.now() + 20 * 60 * 1000;
    let confirming = false;
    while (Date.now() < deadline) {
      const checkAfterReturn = returned || popup.closed;
      wait.signal.throwIfAborted();
      const { status } = await paymentRequest("/payment-status", {}, wait.signal);
      wait.signal.throwIfAborted();
      if (status === "paid") return;
      if (status === "awaiting_approval" && checkAfterReturn) {
        throw new CheckoutDismissed("Payment was not approved. You can try again with the same build settings.");
      }
      if (status === "pending" && !confirming) {
        confirming = true;
        onConfirming?.();
      }
      // A poll started before the return can contain stale approval data.
      if (!checkAfterReturn && (returned || popup.closed)) continue;
      await new Promise(resolve => {
        const timer = setTimeout(resolve, 3500);
        wake = () => { clearTimeout(timer); resolve(); };
      });
      wake = null;
    }
    throw new Error("Checkout timed out. Submit again to check the same order.");
  } catch (error) {
    wait.signal.throwIfAborted();
    throw error;
  } finally {
    window.removeEventListener("focus", backToBuilder);
    signal?.removeEventListener("abort", stopWaiting);
    channel?.close();
    popup.close();
  }
}

export function completeCheckout() {
  if (currentPlan) sessionStorage.removeItem(currentPlan);
  currentKey = currentPlan = null;
}
