import { request } from "./api.js";

function credential(value) {
  if (typeof value !== "string" || !value || value.length > 4096 || /[\r\n]/.test(value)) {
    throw new Error("The server did not return valid payment authorization.");
  }
  return value;
}

export function openPaymentWindow() {
  const popup = window.open("about:blank", "_blank", "popup,width=600,height=750");
  if (!popup) throw new Error("Allow popups to open payment.");
  popup.opener = null;
  return popup;
}

// The browser never signs grants or decides whether provider approval is payment.
// The backend must verify capture, amount, currency, payee, and the exact input.
export async function authorizePayment(file, { onCounts, popup } = {}, transport = request,
                                       pause = ms => new Promise(resolve => setTimeout(resolve, ms))) {
  try {
    const checkout = await transport("/checkout", {
      method: "POST", headers: { "Content-Type": "chemical/x-mdl-sdfile" }, body: file,
    });
    onCounts?.(checkout.counts);
    if (checkout.mode === "test") {
      const payment = checkout.payment_token ? checkout : await transport("/test-payment", {
        method: "POST", headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ checkout_token: credential(checkout.checkout_token) }),
      });
      return credential(payment.payment_token);
    }
    if (checkout.mode !== "redirect") throw new Error("The server has not configured a payment provider.");
    const url = new URL(checkout.url);
    if (url.protocol !== "https:" || url.username || url.password) throw new Error("The payment address must use HTTPS.");
    const checkoutToken = credential(checkout.checkout_token);
    if (!popup || popup.closed) throw new Error("Allow popups and submit again to open payment.");
    popup.location.replace(url.href);
    const deadline = performance.now() + 15 * 60 * 1000;
    while (performance.now() < deadline) {
      const payment = await transport("/payment-status", {
        method: "POST", headers: { Authorization: `Bearer ${checkoutToken}` },
      });
      if (payment.status === "paid") return credential(payment.payment_token);
      if (payment.status === "cancelled" || payment.status === "expired") throw new Error("Payment was cancelled or expired.");
      if (payment.status !== "pending") throw new Error("The server returned an invalid payment status.");
      await pause(2000);
    }
    throw new Error("Payment confirmation timed out. No computation was submitted.");
  } finally {
    popup?.close();
  }
}
