import { previewFile, clearPreview } from "./preview.js";
import { apiBase, request, compute } from "./api.js";
import { authorizePayment, openPaymentWindow } from "./payment.js";

const MAX_BYTES = 10 * 1024 * 1024;

const $ = id => document.getElementById(id);
const input = $("sdf");
const submit = $("submit");
const steps = ["file", "compute", "save"];
let file = null;
let available = false;
let busy = false;
let paymentToken = null;
let paymentMode = "unconfigured";
let computeTimeout = 300;
let selection = 0;
let downloadUrl;
let saved = false;

function showStep(next, focus = true) {
  for (const name of steps) {
    $(`step-${name}`).hidden = name !== next;
    const marker = document.querySelector(`[data-step="${name}"]`);
    if (name === next) marker.setAttribute("aria-current", "step");
    else marker.removeAttribute("aria-current");
  }
  updateControls();
  if (focus) $(`step-${next}`).querySelector("h2").focus();
}

function updateControls() {
  input.disabled = busy || Boolean(downloadUrl);
  submit.disabled = busy || Boolean(downloadUrl) || !available || !file;
  $("compute-progress").hidden = !busy;
  $("compute-note").hidden = !busy;
  if (!busy) $("molecule-progress").hidden = true;
  $("reset").hidden = !downloadUrl || !saved;
}

function status(message, error = false) {
  $("job-status").textContent = message;
  $("job-status").hidden = !message;
  $("job-status").classList.toggle("error", error);
}

function progress(completed, total) {
  if (total > 1) {
    $("compute-progress").max = total;
    $("compute-progress").value = completed;
    $("molecule-progress").textContent = `${completed} / ${total} molecules`;
    $("molecule-progress").hidden = false;
  }
  if (completed === total) status("Receiving ZIP…");
}

function warnBeforeLeaving(event) {
  event.preventDefault();
  event.returnValue = true;
}

function updateSaveState() {
  // Clicking Save is treated as saved; no separate confirmation is required.
  window.removeEventListener("beforeunload", warnBeforeLeaving);
  if (downloadUrl && !saved) window.addEventListener("beforeunload", warnBeforeLeaving);
  $("unsaved-warning").hidden = saved;
  updateControls();
}

function renderCounts(counts) {
  $("molecules").textContent = counts.molecules.toLocaleString();
  $("atoms").textContent = counts.atoms.toLocaleString();
  $("summary").hidden = false;
}

function clearSubmission(clearInput = true) {
  ++selection;
  clearPreview();
  paymentToken = null;
  busy = false;
  file = null;
  if (clearInput) input.value = "";
  for (const id of ["summary", "download", "retry", "back-after-error", "reset", "file-error"]) $(id).hidden = true;
  if (downloadUrl) URL.revokeObjectURL(downloadUrl);
  downloadUrl = null;
  saved = false;
  status("");
  updateSaveState();
}

input.addEventListener("change", async () => {
  if (busy || downloadUrl) return;
  const candidate = input.files[0];
  clearSubmission(false);
  const current = selection;
  if (!candidate) return;
  try {
    if (!/\.(sdf|sd)$/i.test(candidate.name)) throw new Error("Choose an .sdf or .sd file.");
    if (candidate.size > MAX_BYTES) throw new Error("The file exceeds the 10 MiB size limit.");
    const counts = await previewFile(candidate);
    if (current !== selection) return;
    file = candidate;
    renderCounts(counts);
    $("summary").hidden = false;
  } catch (error) {
    if (current !== selection) return;
    clearPreview();
    $("file-error").textContent = error.message;
    $("file-error").hidden = false;
  }
  updateControls();
});

async function continueSubmission() {
  if (busy || downloadUrl || !available || !file) return;
  let popup;
  try {
    if (!paymentToken && paymentMode === "redirect") popup = openPaymentWindow();
  } catch (error) { status(error.message, true); return; }
  busy = true;
  $("compute-progress").removeAttribute("value");
  $("molecule-progress").hidden = true;
  $("retry").hidden = true;
  $("back-after-error").hidden = true;
  showStep("compute");
  try {
    if (!paymentToken) {
      status(paymentMode === "test" ? "Checking file…" : "Waiting for payment…");
      paymentToken = await authorizePayment(file, { onCounts: renderCounts, popup });
    }
    status("Assigning parameters…");
    const result = await compute(file, paymentToken, computeTimeout, progress);
    downloadUrl = URL.createObjectURL(result.blob);
    $("download").href = downloadUrl;
    $("download").download = "molecules.zip";
    $("download").hidden = false;
    paymentToken = null;
    busy = false;
    updateSaveState();
    showStep("save");
    status(result.status === "partial" ? "Some molecules failed. Check their logs in the ZIP." : "");
  } catch (error) {
    busy = false;
    if (error.status === 402) paymentToken = null;
    const message = error.name === "TimeoutError" ? "The request timed out."
      : error instanceof TypeError ? "Connection lost." : error.message;
    status(message, true);
    $("retry").hidden = false;
    $("back-after-error").hidden = false;
    showStep("compute");
  }
}

submit.addEventListener("click", continueSubmission);
$("retry").addEventListener("click", continueSubmission);
$("back-after-error").addEventListener("click", () => {
  if (!busy && !downloadUrl) {
    clearSubmission();
    showStep("file");
  }
});

$("download").addEventListener("click", () => {
  if (!downloadUrl) return;
  saved = true;
  updateSaveState();
});
$("reset").addEventListener("click", () => {
  if (!downloadUrl || !saved) return;
  clearSubmission();
  showStep("file");
});

async function connect() {
  const forceField = $("force-field-download");
  forceField.closest("footer").hidden = !apiBase;
  if (apiBase) forceField.href = `${apiBase}/force-field`;
  if (!apiBase) {
    $("connection").textContent = "Submissions are unavailable.";
    return;
  }
  try {
    const health = await request("/health");
    available = health.ready;
    paymentMode = health.payment_mode;
    $("connection").textContent = available ? "" : "Submissions are unavailable.";
    $("connection").hidden = available;
    computeTimeout = health.compute_timeout_seconds;
    // Older versions stored job IDs here. This adapter has no retained jobs.
    try { sessionStorage.removeItem(`param-gen-job:${apiBase}`); } catch { /* Optional storage. */ }
    updateControls();
  } catch {
    $("connection").textContent = "Cannot connect. Reload the page to try again.";
  }
}
showStep("file", false);
connect();
