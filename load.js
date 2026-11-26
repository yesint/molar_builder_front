import { builderApi, structureUrl } from "./builder-api.js";
import { loadPreparedProtein, setLigands, formatResidueLabel } from "./protein.js";

const $ = id => document.getElementById(id);
const separateLigands = [];
const selectedEnvironment = new Set();
const api = builderApi;
export const loadState = { done: false, busy: false, hasFile: false, hasStructure: false };
let proteinChains = new Set(), proteinCustom = false, ligandCustom = false, selectionTimer = null, selectionVersion = 0;
let customInitialized = { protein: false, ligand: false };
let loaded = null, selected = new Set(), locked = false, frame = null, bytes = null, nextPage = "#membrane";

function error(message = "") { $("load-error").textContent = message; $("load-error").hidden = !message; }
function busy(value, message = "") {
  loadState.busy = value;
  $("add-ligand-files").disabled = value || locked;
  $("load-structure").disabled = value || locked;
  $("load-options").disabled = value || locked || !loadState.hasFile;
  $("partition-system").disabled = value || locked;
  $("load-status").textContent = message; $("load-status").hidden = !message;
  $("load-progress").hidden = !value;
  window.dispatchEvent(new Event("load-state"));
}
function invalidate() {
  loadState.done = false; $("load-ready").hidden = true;
  loadPreparedProtein(null); setLigands([]);
  $("protein-ready").hidden = true; $("ligand-ready").hidden = true;
  window.dispatchEvent(new Event("load-state"));
}
function paged(items, makeRow) {
  const host = document.createElement("div"); let page = 0;
  function render() {
    host.replaceChildren(...items.slice(page * 100, page * 100 + 100).map(makeRow));
    if (items.length <= 100) return;
    const nav = document.createElement("div"); nav.className = "actions";
    for (const [label, delta] of [["Previous", -1], ["Next", 1]]) {
      const button = document.createElement("button"); button.type = "button"; button.className = "secondary compact";
      button.textContent = label; button.disabled = page + delta < 0 || (page + delta) * 100 >= items.length;
      button.onclick = () => { page += delta; render(); }; nav.append(button);
    }
    host.append(nav);
  }
  render(); return host;
}
function syncLigandChoices() {
  const all = $("select-all-ligands");
  const residueCount = ligandCustom ? 0 : (loaded?.ligands.length || 0) + (loaded?.other?.length || 0);
  const total = residueCount + separateLigands.length;
  const count = (ligandCustom ? 0 : selected.size) + separateLigands.filter(item => item.selected).length;
  all.parentElement.hidden = total === 0;
  all.nextElementSibling.textContent = ligandCustom ? "Select all external ligands" : "Select all ligands";
  all.checked = total > 0 && count === total;
  all.indeterminate = count > 0 && count < total;
  for (const box of $("ligand-residues-panel").querySelectorAll("input[type=checkbox]")) {
    box.checked = box.residueIds.every(id => selected.has(id));
    box.indeterminate = !box.checked && box.residueIds.some(id => selected.has(id));
  }
  $("separate-ligands").querySelectorAll("input").forEach((box, i) => { box.checked = separateLigands[i].selected; });
}
function choice(text, ids) {
  const label = document.createElement("label"); label.className = "protein-check";
  const input = document.createElement("input"); input.type = "checkbox";
  input.residueIds = ids;
  input.checked = ids.every(id => selected.has(id)); input.indeterminate = !input.checked && ids.some(id => selected.has(id));
  input.onchange = () => {
    for (const id of ids) input.checked ? selected.add(id) : selected.delete(id);
    invalidate();
    syncLigandChoices();
    updateViewer();
  };
  label.append(input, document.createTextNode(text)); return label;
}
function groups(items) {
  const names = new Map();
  for (const r of items) { if (!names.has(r.name)) names.set(r.name, []); names.get(r.name).push(r); }
  return paged([...names], ([name, residues]) => {
    const host = document.createElement("div");
    const title = residues.length === 1 ? formatResidueLabel(residues[0], true) : `${name} (${residues.length})`;
    host.append(choice(title, residues.map(r => r.id)));
    if (residues.length > 1) {
      const details = document.createElement("details"), summary = document.createElement("summary"), rows = document.createElement("div");
      summary.textContent = "Residues";
      details.ontoggle = () => rows.replaceChildren(...(details.open ? [paged(residues, r => choice(formatResidueLabel(r, true), [r.id]))] : []));
      details.append(summary, rows); host.append(details);
    }
    return host;
  });
}
function render() {
  $("load-chains").replaceChildren(...loaded.chains.map(c => {
    const row = document.createElement("label"); row.className = "protein-check";
    const input = document.createElement("input"); input.type = "checkbox"; input.checked = proteinChains.has(c.id);
    input.onchange = () => { input.checked ? proteinChains.add(c.id) : proteinChains.delete(c.id); invalidate(); updateViewer(); };
    row.append(input, document.createTextNode(c.id.trim() ? `Chain ${c.id.trim()}` : "No chain ID")); return row;
  }));
  $("load-ligands").replaceChildren(groups([...loaded.ligands, ...(loaded.other || [])]));
  $("load-environment-section").hidden = !loaded.environment?.length;
  $("load-environment").replaceChildren(paged(loaded.environment || [], item => {
    const row = document.createElement("label"), box = document.createElement("input");
    row.className = "protein-check"; box.type = "checkbox"; box.checked = selectedEnvironment.has(item.name);
    box.onchange = () => {
      if (box.checked) selectedEnvironment.add(item.name); else selectedEnvironment.delete(item.name);
      invalidate(); updateViewer();
    };
    row.append(box, document.createTextNode(`${item.name}, ${item.category.replaceAll("_", " ")} (${item.residues})`));
    return row;
  }));
}
const proteinSelection = () => {
  if (!loaded) return "";
  if (proteinCustom) return $("component-protein-selection").value.trim();
  const ids = loaded.residues.filter(r => r.protein && proteinChains.has(r.chain)).map(r => r.preview_index ?? r.id);
  if (!ids.length) return "";
  if (proteinChains.size === loaded.chains.length) return loaded.protein_selection || `resindex ${ids.join(" ")}`;
  return `resindex ${ids.join(" ")}`;
};
const ligandSelection = () => {
  if (!loaded) return "";
  if (ligandCustom) return $("component-ligand-selection").value.trim();
  const ids = loaded.residues.filter(r => selected.has(r.id)).map(r => r.preview_index ?? r.id);
  const parts = ids.length ? [`resindex ${ids.join(" ")}`] : [];
  if (selectedEnvironment.size) {
    let group = `resname ${[...selectedEnvironment].join(" ")}`;
    const exceptions = loaded.residues.filter(r => selectedEnvironment.has(r.name)).map(r => r.preview_index ?? r.id);
    if (exceptions.length) group = `(${group} and not (resindex ${exceptions.join(" ")}))`;
    parts.push(group);
  }
  return parts.join(" or ");
};
function externalLigands() {
  return separateLigands.map(item => ({ key: item.structure_url, file: item.file, selected: item.selected }));
}
function openViewer(preview = null) {
  bytes = preview;
  frame = document.createElement("iframe"); frame.title = "Loaded system"; frame.src = "./viewer.html";
  $("load-viewer").hidden = false;
  $("load-viewer").replaceChildren(frame);
}
function updateViewer() {
  error(); selectionVersion++;
  frame?.contentWindow.postMessage({ type: "protein-selection", requestId: selectionVersion, protein: proteinSelection(), ligands: ligandSelection(), externalLigands: externalLigands() }, location.origin);
}
function setMode(component, custom, notify = true) {
  const list = component === "protein" ? "chains" : "residues";
  const before = component === "protein" ? proteinSelection() : ligandSelection();
  if (custom && !customInitialized[component]) {
    $("component-" + component + "-selection").value = component === "protein" ? proteinSelection() : ligandSelection();
    customInitialized[component] = true;
  }
  if (component === "protein") proteinCustom = custom; else ligandCustom = custom;
  for (const [mode, active] of [[list, !custom], ["custom", custom]]) {
    const tab = $(component + "-" + mode + "-tab");
    tab.setAttribute("aria-selected", String(active)); tab.tabIndex = active ? 0 : -1;
    $(component + "-" + mode + "-panel").hidden = !active;
  }
  if (component === "ligand") {
    syncLigandChoices();
    $("load-environment-section").hidden = custom || !loaded?.environment?.length;
  }
  if (notify) {
    if (before !== (component === "protein" ? proteinSelection() : ligandSelection())) invalidate();
    updateViewer();
  }
}
async function checked(response) {
  const result = await response.json(); if (!response.ok) throw new Error(result.error || `Load failed (HTTP ${response.status}).`); return result;
}

export function setupLoad() {
  for (const component of ["protein", "ligand"]) {
    const list = component === "protein" ? "chains" : "residues";
    for (const mode of [list, "custom"]) {
      const tab = $(component + "-" + mode + "-tab");
      tab.onclick = () => setMode(component, mode === "custom");
      tab.onkeydown = event => {
        if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
        event.preventDefault();
        const custom = event.key === "End" || (event.key !== "Home" && mode !== "custom");
        setMode(component, custom); $(component + "-" + (custom ? "custom" : list) + "-tab").focus();
      };
    }
    $("component-" + component + "-selection").oninput = () => {
      invalidate(); selectionVersion++; clearTimeout(selectionTimer);
      selectionTimer = setTimeout(updateViewer, 250);
    };
  }
  $("select-all-ligands").onchange = event => {
    const checked = event.target.checked;
    if (!ligandCustom) {
      selected.clear();
      if (checked && loaded) {
        for (const residue of [...loaded.ligands, ...(loaded.other || [])]) selected.add(residue.id);
      }
    }
    for (const item of separateLigands) item.selected = checked;
    syncLigandChoices(); invalidate(); updateViewer();
  };
  function refreshFiles() {
    loadState.hasFile = Boolean(loaded || separateLigands.length);
    $("load-actions").hidden = !loadState.hasFile;
    $("separate-ligands").replaceChildren(...separateLigands.map(item => {
      const row = document.createElement("label"), box = document.createElement("input");
      row.className = "protein-check"; box.type = "checkbox"; box.checked = item.selected;
      box.onchange = () => { item.selected = box.checked; syncLigandChoices(); invalidate(); updateViewer(); };
      row.append(box, document.createTextNode(item.label)); return row;
    }));
    syncLigandChoices();
    $("load-options").hidden = false;
    $("load-protein-section").hidden = !loaded;
    $("ligand-residues-tab").parentElement.hidden = !loaded;
    if (!frame) openViewer(); else updateViewer();
    invalidate();
  }
  $("add-ligand-files").onclick = () => $("ligand-files").click();
  $("ligand-files").onchange = async () => {
    const records = [];
    busy(true, "Reading ligands…"); error();
    try {
      for (const file of $("ligand-files").files) {
        const data = new Uint8Array(await file.arrayBuffer());
        // Match record boundaries using byte offsets to retain the uploaded bytes.
        const text = new TextDecoder("latin1").decode(data);
        let start = 0;
        const ends = [...text.matchAll(/^\$\$\$\$[^\S\r\n]*(?:\r?\n|$)/gm)].map(m => m.index + m[0].length);
        if (text.slice(ends.at(-1) || 0).trim()) ends.push(data.length);
        if (!ends.length) throw new Error(file.name + " contains no SDF records.");
        for (const [i, end] of ends.entries()) {
          if (!/^M  END\s*$/m.test(text.slice(start, end))) throw new Error(file.name + " contains an invalid molfile.");
          const label = ends.length === 1 ? file.name : `${file.name} (mol ${i + 1})`;
          records.push({ label, file: ends.length === 1 ? file : file.slice(start, end) });
          start = end;
        }
      }
      for (const {label, file} of records) separateLigands.push({label, file, selected: true,
        structure_url: URL.createObjectURL(file), residue: {name: label, number: "", chain: ""}});
      refreshFiles();
    } catch (cause) { error(cause.message); }
    finally { busy(false); $("ligand-files").value = ""; }
  };
  $("load-structure").onclick = () => $("structure-file").click();
  $("structure-file").onchange = async () => {
    const file = $("structure-file").files[0]; if (!file) return;
    busy(true, "Analyzing structure…"); error();
    try {
      const extension = file.name.split(".").pop().toLowerCase();
      if (!["pdb", "cif", "mmcif"].includes(extension)) throw new Error("Load a PDB or mmCIF structure.");
      const report = await checked(await fetch(`${api}/load/structure`, { method: "POST", headers: {
        "Content-Type": "application/octet-stream", "X-Structure-Format": extension }, body: file, signal: AbortSignal.timeout(630000) }));
      const response = await fetch(structureUrl(report.structure_url)); if (!response.ok) throw new Error("The loaded structure is no longer available.");
      const preview = await response.arrayBuffer();
      loadState.hasStructure = true; loadState.hasFile = true;
      invalidate(); loaded = report; selectedEnvironment.clear();
      const standalone = report.other?.length === 1 && !report.chains.length &&
        !report.ligands.length && !report.environment?.length;
      selected = new Set((standalone ? report.other : report.ligands).map(r => r.id));
      proteinChains = new Set(report.chains.map(c => c.id));
      customInitialized = { protein: false, ligand: false };
      $("component-protein-selection").value = ""; $("component-ligand-selection").value = "";
      setMode("protein", false, false); setMode("ligand", false, false); render();
      $("structure-filename").textContent = file.name;
      for (const id of ["load-options", "load-actions", "load-viewer"]) $(id).hidden = false;
      $("load-protein-section").hidden = false; $("ligand-residues-tab").parentElement.hidden = false;
      openViewer(preview);
    } catch (cause) { error(cause.message); }
    finally { busy(false); $("structure-file").value = ""; }
  };
  $("partition-system").onclick = async () => {
    if ((!loaded && !separateLigands.length) || locked || loadState.busy) return;
    if (loadState.done) { location.hash = nextPage; return; }
    busy(true, "Partitioning system…"); error();
    try {
      const result = loaded ? await checked(await fetch(`${api}/load/partition`, { method: "POST", headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ upload_id: loaded.upload_id, protein_selection: proteinSelection(), ligand_selection: ligandSelection() }) })) : {protein: null, ligands: []};
      await loadPreparedProtein(result.protein);
      const ligands = [...result.ligands, ...separateLigands.filter(item => item.selected)];
      setLigands(ligands);
      loadState.done = true; $("load-ready").hidden = false;
      nextPage = result.protein?.chains.length ? "#proteins" : ligands.length ? "#ligands" : "#membrane";
      location.hash = nextPage;
    } catch (cause) { error(cause.message); }
    finally { busy(false); }
  };
  window.addEventListener("preparation-lock", event => { locked = event.detail; busy(loadState.busy); });
  window.addEventListener("message", event => {
    if (event.origin !== location.origin || event.source !== frame?.contentWindow) return;
    if (event.data?.type === "ready") {
      frame.contentWindow.postMessage({ type: "file", mode: "protein", name: "loaded.cif", bytes,
        protein: proteinSelection(), proteinStyle: loaded?.coarse_grained ? "vdw" : "cartoon",
        ligandStyle: loaded?.chains.length ? "licorice" : "ballstick", ligands: ligandSelection(), externalLigands: externalLigands() }, location.origin, bytes ? [bytes] : []); bytes = null;
    } else if (event.data?.type === "rendered") {
      $("load-viewer").dataset.state = "rendered"; updateViewer();
    } else if (event.data?.type === "selection-error" && event.data.requestId === selectionVersion) {
      error(event.data.message);
    } else if (["invalid", "unavailable"].includes(event.data?.type)) error(event.data.message || "The structure viewer is unavailable.");
  });
}
