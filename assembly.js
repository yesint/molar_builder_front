import { buildHeaders, CheckoutDismissed, completeCheckout } from "./builder-checkout.js";
import { builderApi } from "./builder-api.js";
import { setProteinLocked } from "./protein.js";
const $ = id => document.getElementById(id);
let solventConfirmed = false;
let withMembrane = true, resultURL = null, busy = false;
let workflow = { ready: () => false };
let activeStage = null, cancelPreview = null, viewerGeneration = 0, completed = null;

const api = builderApi;

function warnBeforeLeaving(event) {
  event.preventDefault();
  event.returnValue = true;
}

function showDownload(blob) {
  const button = $("assembly-download");
  if (resultURL) URL.revokeObjectURL(resultURL);
  resultURL = URL.createObjectURL(blob);
  $("assembly-archive").href = resultURL;
  button.textContent = "Save ZIP";
  button.hidden = false;
  $("assembly-unsaved-warning").hidden = false;
  window.addEventListener("beforeunload", warnBeforeLeaving);
}

function archiveNotice(message) {
  $("archive-notice").textContent = message;
  $("archive-notice").hidden = !message;
}

async function releaseBuild(result) {
  if (!result.structure || result.released) return;
  try {
    const response = await fetch(`${api}/assembly/${result.id}/received`, {
      method: "POST", headers: {"Content-Type": "application/json"}, body: "{}", signal: AbortSignal.timeout(10000),
    });
    if (!response.ok) throw new Error();
    result.released = true;
    if (completed === result) archiveNotice("");
  } catch {
    if (completed === result) archiveNotice("The ZIP is available in this tab. The server could not confirm cleanup.");
  }
}

function logEntry(parent, label, started) {
  const row = document.createElement("li"), line = document.createElement("div");
  const mark = document.createElement("span"), name = document.createElement("span"), time = document.createElement("time");
  row.dataset.state = "running"; line.className = "log-line";
  mark.className = "stage-mark"; mark.textContent = "…";
  name.textContent = label; line.append(mark, name, time); row.append(line); parent.append(row);
  const log = $("assembly-log");
  log.scrollTop = log.scrollHeight;
  return { row, mark, label, time, started };
}
function finish(entry, success, end) {
  if (!entry) return;
  entry.row.dataset.state = success ? "done" : "failed"; entry.mark.textContent = success ? "✓" : "×";
  setDuration(entry.time, end - entry.started);
}
function setDuration(time, elapsed) {
  const seconds = Math.max(0, elapsed) / 1000;
  time.dateTime = `PT${seconds.toFixed(3)}S`;
  time.textContent = seconds < 60 ? `${seconds.toFixed(2)} s` : `${Math.floor(seconds / 60)} min ${Math.floor(seconds % 60)} s`;
}
function finishStage(success = true) {
  if (!activeStage) return;
  const end = performance.now();
  finish(activeStage.current, success, end); finish(activeStage, success, end);
  activeStage = null;
}
function task(label, at = performance.now()) {
  if (!activeStage) return;
  if (!activeStage.tasks) {
    activeStage.tasks = document.createElement("ol");
    activeStage.row.append(activeStage.tasks);
  }
  finish(activeStage.current, true, at);
  activeStage.current = logEntry(activeStage.tasks, label.replace(/…$/, ""), at);
  $("assembly-status").textContent = `${activeStage.label}: ${activeStage.current.label}`;
}
function stage(label, firstTask) {
  finishStage();
  $("assembly-log").hidden = false;
  activeStage = logEntry($("assembly-log"), label, performance.now());
  $("assembly-status").textContent = label;
  if (firstTask) task(firstTask);
}
async function requestWithProgress(url, options) {
  const token = Array.from(crypto.getRandomValues(new Uint8Array(16)), n => n.toString(16).padStart(2, "0")).join("");
  const endpoint = `${api}/build-progress/${token}`;
  const owner = activeStage, start = performance.now();
  let count = 0, stopped = false, timer, pending;
  async function read() {
    try {
      const response = await fetch(endpoint, { cache: "no-store", signal: AbortSignal.timeout(3000) });
      if (!response.ok || owner !== activeStage) return;
      const events = await response.json();
      if (owner !== activeStage) return;
      for (const [ms, label] of events.slice(count)) {
        task(label, Math.min(performance.now(), start + ms)); count++;
      }
    } catch { /* The compute response carries the build result or error. */ }
  }
  function schedule() {
    if (!stopped) timer = setTimeout(() => { pending = read().then(schedule); }, 350);
  }
  schedule();
  try { return await fetch(url, { ...options, headers: buildHeaders({ ...options.headers, "X-Build-Progress": token }) }); }
  finally { stopped = true; clearTimeout(timer); await pending; await read(); }
}

async function showFinalSystem(result) {
  const generation = ++viewerGeneration;
  const options = workflow.viewer();
  if (!result.structure) {
    const response = await fetch(`${api}/assembly/${result.id}/system.cif`);
    if (!response.ok) throw new Error(`Loading assembled coordinates failed (HTTP ${response.status}).`);
    result.structure = await response.blob();
  }
  await releaseBuild(result);
  const bytes = await result.structure.arrayBuffer();
  if (generation !== viewerGeneration) return;
  const host = $("assembly-viewer"), frame = document.createElement("iframe");
  frame.title = "Assembled system";
  frame.src = "./viewer.html";
  host.hidden = false;
  host.dataset.state = "loading";
  await new Promise((resolve, reject) => {
    let timer, transferred = false, phase = "Starting viewer";
    // MolAR processes scene commands on animation frames. A background tab or
    // an iframe outside the viewport can stop receiving those frames.
    let visible = false;
    function armTimer() {
      clearTimeout(timer);
      if (visible && !document.hidden) timer = setTimeout(() => {
        done(new Error(`MolAR stopped responding while ${phase.toLowerCase()}.`));
      }, 120000);
    }
    const observer = new IntersectionObserver(([entry]) => {
      visible = entry.isIntersecting;
      armTimer();
    });
    function done(error) {
      clearTimeout(timer);
      observer.disconnect();
      document.removeEventListener("visibilitychange", armTimer);
      window.removeEventListener("message", receive);
      cancelPreview = null;
      if (error) { host.dataset.state = "unavailable"; reject(error); }
      else resolve();
    }
    function receive(event) {
      if (event.origin !== location.origin || event.source !== frame.contentWindow) return;
      if (event.data?.type === "ready" && !transferred) {
        transferred = true;
        try {
          frame.contentWindow.postMessage({type: "file", mode: "assembly", name: "system.cif", bytes, ...options}, location.origin, [bytes]);
        } catch (error) { done(error); }
      } else if (event.data?.type === "viewer-progress") {
        phase = event.data.message;
        armTimer();
      } else if (event.data?.type === "rendered") {
        host.dataset.state = "rendered"; done();
      } else if (["invalid", "unavailable"].includes(event.data?.type)) {
        done(new Error(event.data.message || "The final-system viewer is unavailable."));
      }
    }
    cancelPreview = () => done();
    window.addEventListener("message", receive);
    document.addEventListener("visibilitychange", armTimer);
    host.replaceChildren(frame);
    observer.observe(frame);
    if (location.hash === "#submit") host.scrollIntoView({block: "nearest"});
  });
}

export function setAssemblyMembrane(value) {
  if (withMembrane !== value) solventConfirmed = false;
  withMembrane = value;
  $("waters-per-lipid").hidden = $("waters-per-lipid-label").hidden = !withMembrane;
  $("assembly-box").textContent = withMembrane ? "Box: hexagonal membrane cell." : "Box: rhombic dodecahedron.";
}
export function invalidateAssembly() {
  if (resultURL) $("assembly-download").textContent = "Save previous ZIP";
  $("assembly-ready").hidden = true;
  $("solvent-ready").hidden = !solventConfirmed;
  $("continue-solvent").disabled = !workflow.ready() || busy;
  $("assemble-system").disabled = !workflow.ready() || !solventConfirmed || busy;
  const solvent = settings();
  const steps = [...(workflow.plan?.() || [])];
  if (solvent.add_solvent) {
    const details = [`${solvent.concentration_molar} mol/L ${solvent.salt}`];
    if (solvent.neutralize) details.push("neutralize");
    steps.push(["Add solvent and ions", details.join(", ")]);
  }
  steps.push(["Run final grompp"], ["Load final system"]);
  $("submit-summary").replaceChildren(...steps.map(([label, detail]) => {
    const row = document.createElement("li"); row.textContent = detail ? `${label} (${detail})` : label; return row;
  }));
  $("assemble-system").hidden = false;
  viewerGeneration++;
  cancelPreview?.();
  completed = null;
  $("retry-viewer").hidden = true;
  $("assembly-viewer").replaceChildren(); $("assembly-viewer").hidden = true;
  delete $("assembly-viewer").dataset.state;
  $("build-total").hidden = true;
  activeStage = null; $("assembly-log").replaceChildren(); $("assembly-log").hidden = true;
  $("assembly-error").hidden = true;
}
function settings() {
  if (!$("add-solvent").checked) return { add_solvent: false };
  const common = { add_solvent: true, water_clearance_nm: $("water-clearance").valueAsNumber, salt: $("assembly-salt").value,
    concentration_molar: $("salt-concentration").valueAsNumber, neutralize: $("neutralize-system").checked };
  return withMembrane ? { ...common, waters_per_lipid: $("waters-per-lipid").valueAsNumber } : common;
}
export function setupAssembly(callbacks) {
  workflow = callbacks;
  $("assembly-download").onclick = () => {
    if (!resultURL) return;
    $("assembly-archive").click();
    $("assembly-unsaved-warning").hidden = true;
    window.removeEventListener("beforeunload", warnBeforeLeaving);
    if (completed) void releaseBuild(completed);
  };
  async function previewCompleted() {
    const result = completed;
    $("retry-viewer").hidden = true;
    $("assembly-error").hidden = true;
    stage("Loading final system");
    try {
      await showFinalSystem(result);
      if (completed === result) finishStage();
    } catch (error) {
      if (completed !== result) return;
      finishStage(false);
      $("assembly-error").textContent = error.message;
      $("assembly-error").hidden = false;
      $("retry-viewer").hidden = false;
    }
  }
  $("retry-viewer").onclick = () => {
    $("assembly-log").querySelector('li[data-state="failed"]:last-child')?.remove();
    previewCompleted();
  };
  $("assembly-options").addEventListener("input", () => {
    $("solvent-parameters").hidden = $("solvent-parameters").disabled = !$("add-solvent").checked;
    solventConfirmed = false;
    invalidateAssembly();
  });
  $("continue-solvent").onclick = () => {
    if (busy || !workflow.ready()) return;
    if (![...$("assembly-options").querySelectorAll("input")].filter(input => !input.hidden).every(input => input.reportValidity())) return;
    if (!solventConfirmed) {
      solventConfirmed = true;
      invalidateAssembly();
    }
    location.hash = "#submit";
  };
  $("force-field-download").href = `${api}/assembly/force-field.zip`;
  $("assemble-system").onclick = async () => {
    if (!workflow.ready() || !solventConfirmed || busy) return;
    if (![...$("assembly-options").querySelectorAll("input")].filter(input => !input.hidden).every(input => input.reportValidity())) return;
    const started = performance.now();
    const options = { ...settings(), project_file:
          ($("structure-filename").textContent.replace(/\.[^.]+$/, "").replace(/[^\w.-]/g, "_").slice(0, 150) || "project") + ".toml" };
    invalidateAssembly(); busy = true;
    window.dispatchEvent(new CustomEvent("assembly-busy", { detail: true }));
    setProteinLocked(true);
    $("continue-solvent").disabled = true; $("assemble-system").disabled = true; $("assembly-options").disabled = true;
    $("assembly-progress").hidden = false; $("assembly-status").hidden = false;
    try {
      archiveNotice("");
      const target = await workflow.prepare({stage, task, request: requestWithProgress, assembly: options});
      stage(options.add_solvent ? "Solvating and assembling system" : "Assembling dry system", "Checking assembly inputs");
      const response = await requestWithProgress(`${api}/assembly/compute`, {
        method: "POST", headers: { "Content-Type": "application/json" }, signal: AbortSignal.timeout(1230000),
        body: JSON.stringify({ ...target, ...options }),
      });
      if (!response.ok) { const detail = await response.json().catch(() => ({})); throw new Error(detail.error || `Assembly failed (HTTP ${response.status}).`); }
      const id = response.headers.get("X-Assembly-ID");
      if (!id || !/^[0-9a-f]{32}$/.test(id)) throw new Error("The server did not identify the assembled system.");
      const blob = await response.blob();
      if (!blob.size) throw new Error("The server returned an empty assembly ZIP.");
      showDownload(blob);
      completed = {id, released: false, structure: null};
      $("assembly-ready").hidden = false;
      $("assemble-system").hidden = true;
      completeCheckout();
      void previewCompleted();
    } catch (cause) {
      if (cause instanceof CheckoutDismissed) {
        activeStage = null;
        $("assembly-log").replaceChildren();
        $("assembly-log").hidden = true;
        archiveNotice(cause.message);
        return;
      }
      finishStage(false);
      $("assembly-error").textContent = `${$("assembly-status").textContent.replace(/…$/, "")}: ${cause.message}`;
      $("assembly-error").hidden = false;
    } finally {
      setDuration($("build-duration"), performance.now() - started);
      $("build-total").hidden = $("assembly-log").hidden;
      busy = false; setProteinLocked(false); $("assembly-options").disabled = false;
      $("assembly-progress").hidden = true; $("assembly-status").hidden = true;
      window.dispatchEvent(new CustomEvent("assembly-busy", { detail: false }));
      $("assemble-system").disabled = !workflow.ready() || !solventConfirmed;
      $("continue-solvent").disabled = !workflow.ready();
    }
  };
}
