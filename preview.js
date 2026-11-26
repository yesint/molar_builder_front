// Each preview owns a document so molar_vis can release its renderer on reset.
const panel = document.getElementById("preview");
let active = null;
let generation = 0;

export function clearPreview() {
  ++generation;
  if (active) {
    clearTimeout(active.timer);
    window.removeEventListener("message", active.receive);
    active.reject(new Error("File selection changed."));
    active = null;
  }
  panel.replaceChildren();
  panel.hidden = true;
  delete panel.dataset.state;
  document.querySelector("main").classList.remove("has-preview");
}

export async function previewFile(file) {
  clearPreview();
  const current = generation;
  const bytes = await file.arrayBuffer();
  if (current !== generation) throw new Error("File selection changed.");
  const frame = document.createElement("iframe");
  frame.title = "Molecule preview";
  frame.src = new URL("./viewer.html", import.meta.url);
  return new Promise((resolve, reject) => {
    const receive = event => {
      if (event.origin !== location.origin || event.source !== frame.contentWindow) return;
      const message = event.data;
      if (message?.type === "ready") {
        frame.contentWindow.postMessage({ type: "file", name: file.name, bytes }, location.origin, [bytes]);
      } else if (message?.type === "counts") {
        clearTimeout(active?.timer);
        resolve(message.counts);
      } else if (message?.type === "invalid") {
        clearTimeout(active?.timer);
        reject(new Error(message.message));
      } else if (message?.type === "rendered" || message?.type === "unavailable") {
        panel.dataset.state = message.type;
      }
    };
    const timer = setTimeout(() => reject(new Error("The file reader could not load. Reload the page to try again.")), 60000);
    active = { receive, reject, timer };
    window.addEventListener("message", receive);
    panel.replaceChildren(frame);
    panel.hidden = false;
    document.querySelector("main").classList.add("has-preview");
  });
}
