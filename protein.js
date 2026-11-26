import { buildHeaders } from "./builder-checkout.js";
import { builderApi, structureUrl } from "./builder-api.js";
import { editGlycan, attachmentAnomer, attachmentBond } from "./glycans.js";

const $ = id => document.getElementById(id);
const api = `${builderApi}/protein`;
export const proteinState = { build: null, request: null, pending: false, busy: false };
export const ligandState = { items: [], sources: null, parameters: new Map(), busy: false, pending: false };
let ligandEditing = false;
let loaded = null;
let chains = new Set();
let ptms = [];
const mutations = new Map();
const aminoAcids = "ALA ARG ASN ASP CYS GLN GLU GLY HIS ILE LEU LYS MET PHE PRO SER THR TRP TYR VAL".split(" ");
let bonds = [];
let changed = () => {};
let frame = null;
let frameReady = false;
let displayedBuild = null;
let viewerGeneration = 0;
let loadedBytes = null;
let ligandFrame = null;
let ligandBytes = null;
let currentLigand = null;
let ligandGeneration = 0;
let ligandLoaded = false;
let ligandBusy = false;
let ligandExport = null;
let ligandRequest = 0;
const ligandEdits = new Map();
const originalViewerCifs = new Map();
const editedInViewer = new Set();
let ligandDownload = null;
let locked = false;
let residueById = new Map();
let residueLabelCounts = new Map();

const residueKey = r => `${r.chain}:${r.number}`;
export function formatResidueLabel(r, includeIndex = false) {
  const chain = r.chain.trim();
  return `${r.name} ${chain ? `${chain}:` : ""}${r.number}${includeIndex ? ` (resindex ${r.id})` : ""}`;
}
const residueLabel = r => formatResidueLabel(r, residueLabelCounts.get(residueKey(r)) > 1);
function mutatedResidue(r, name = mutations.get(r.id)) {
  return name ? {...r, name, cysteine: name === "CYS", phosphorylatable: ["SER","THR","TYR"].includes(name),
    glycosylatable: ["ASN","SER","THR"].includes(name)} : r;
}
const residue = id => mutatedResidue(residueById.get(id));
const selected = id => chains.has(residue(id).chain);
const chainSelection = chain => /^[A-Za-z0-9]$/.test(chain) ? `chain ${chain}`
  : `resindex ${loaded.residues.filter(r => r.chain === chain).map(r => r.preview_index ?? r.id).join(" ")}`;
function error(message) { $("protein-error").textContent = message; $("protein-error").hidden = !message; }

function proteinSelection() {
  if (!chains.size) return "not all";
  const protein = loaded.protein_selection || "protein";
  if (chains.size === loaded.chains.length) return protein;
  return `(${protein}) and (${[...chains].map(chain => `(${chainSelection(chain)})`).join(" or ")})`;
}

function detailSelection(ids, built = true) {
  const preparedResidues = loaded.residues.filter(r => r.protein && chains.has(r.chain));
  const indices = [...new Set(ids)].filter(selected).map(id => built
    ? preparedResidues.findIndex(r => r.id === id) : (residue(id).preview_index ?? id));
  return indices.length ? `resindex ${indices.join(" ")}` : "";
}

function previewDetails() {
  const built = !!displayedBuild;
  return {
    ptms: detailSelection([...loaded.ptms.map(r => r.id), ...ptms.map(p => p.residue)], built),
    disulfides: detailSelection(bonds.filter(b => b.enabled && b.residues.every(selected)).flatMap(b => b.residues), built),
  };
}
function updatePreview() {
  if (frameReady) frame.contentWindow.postMessage({
    type: "protein-selection", protein: displayedBuild?.protein_selection || proteinSelection(), ...previewDetails(),
  }, location.origin);
}

async function preview(url, build = null) {
  displayedBuild = build;
  const generation = ++viewerGeneration;
  frameReady = false;
  $("protein-viewer").dataset.state = "loading";
  $("protein-viewer").replaceChildren();
  const response = await fetch(structureUrl(url));
  if (!response.ok) throw new Error("The protein structure could not be loaded.");
  const bytes = await response.arrayBuffer();
  if (generation !== viewerGeneration) return;
  loadedBytes = bytes;
  frame = document.createElement("iframe");
  frame.title = build ? "Prepared protein" : "Protein structure";
  frame.src = "./viewer.html";
  $("protein-viewer").append(frame);
}

function markChanged() {
  proteinState.build = null; proteinState.request = null;
  proteinState.pending = !!loaded && chains.size > 0;
  $("protein-built").hidden = true;
  $("protein-ready").hidden = proteinState.pending || !loaded;
  error("");
  updatePreview();
  changed();
}

function checkbox(label, checked, onChange, suffix = "") {
  const item = document.createElement("label");
  item.className = "protein-check";
  const input = document.createElement("input");
  input.type = "checkbox"; input.checked = checked;
  input.addEventListener("change", () => onChange(input.checked));
  const name = document.createElement("span"); name.textContent = label;
  item.append(input, name);
  if (suffix) { const detail = document.createElement("span"); detail.className = "small"; detail.textContent = suffix; item.append(detail); }
  return item;
}

function removable(label, remove) {
  const item = document.createElement("div"); item.className = "protein-item";
  const text = document.createElement("span"); text.textContent = label;
  const button = document.createElement("button"); button.type = "button"; button.className = "secondary remove-lipid";
  button.textContent = "×"; button.setAttribute("aria-label", `Remove ${label}`); button.addEventListener("click", remove);
  item.append(text, button); return item;
}

function paged(items, row) {
  const host = document.createElement("div");
  let page = 0;
  function draw() {
    host.replaceChildren(...items.slice(page * 100, (page + 1) * 100).map(row));
    if (items.length <= 100) return;
    const controls = document.createElement("div"); controls.className = "actions";
    for (const [label, delta] of [["Previous", -1], ["Next", 1]]) {
      const button = document.createElement("button"); button.type = "button"; button.className = "secondary compact";
      button.textContent = label; button.disabled = page + delta < 0 || (page + delta) * 100 >= items.length;
      button.addEventListener("click", () => { page += delta; draw(); }); controls.append(button);
    }
    const position = document.createElement("span"); position.textContent = `${page * 100 + 1}–${Math.min((page + 1) * 100, items.length)} / ${items.length}`;
    controls.append(position); host.append(controls);
  }
  draw(); return host;
}

const cysteineLipidTypes = new Set(["palmitoylation", "farnesylation", "geranylgeranylation", "triacylation"]);
const freeCysteine = r => r.name === "CYS" && !bonds.some(b => b.enabled && b.residues.includes(r.id));
const lipidatedCysteine = id => ptms.some(p => p.residue === id && cysteineLipidTypes.has(p.type));
const ptmTypes = {
  phosphorylation: { label: "Phosphorylation", eligible: r => r.phosphorylatable },
  palmitoylation: { label: "S-palmitoylation", eligible: freeCysteine },
  glycine_myristoylation: { label: "N-myristoylation", eligible: r => r.name === "GLY" && r.n_terminal },
  lysine_myristoylation: { label: "Lys myristoylation", eligible: r => r.name === "LYS" },
  farnesylation: { label: "S-farnesylation", eligible: freeCysteine },
  geranylgeranylation: { label: "S-geranylgeranylation", eligible: freeCysteine },
  triacylation: { label: "Triacylation", eligible: r => freeCysteine(r) && r.n_terminal },
  glycosylation: { label: "Glycosilation", eligible: r => r.glycosylatable ?? ["ASN", "SER", "THR"].includes(r.name) },
};
function eligiblePtmResidues(type) {
  return loaded.residues.map(r => mutatedResidue(r)).filter(r => chains.has(r.chain) && !r.coarse_grained && ptmTypes[type].eligible(r) && !ptms.some(p => p.residue === r.id));
}
function compatibleMutation(id, next) {
  const ptm = ptms.find(p => p.residue === id);
  if (ptm && !ptmTypes[ptm.type].eligible(next)) {
    error("Remove the incompatible PTM before changing this residue."); return false;
  }
  if (bonds.some(b => b.enabled && b.residues.includes(id)) && next.name !== "CYS") {
    error("Disable the disulfide bond before changing this cysteine."); return false;
  }
  return true;
}
function updatePtmDialog() {
  const type = $("ptm-type").value;
  const terms = $("ptm-search").value.toLowerCase().trim().split(/\s+/);
  fillResidues($("ptm-residue"), eligiblePtmResidues(type).filter(r =>
    terms.every(term => /^[a-z]$/.test(term) ? r.chain.trim().toLowerCase() === term
      : formatResidueLabel(r).toLowerCase().includes(term))));
  $("ptm-charge").hidden = $("ptm-charge-label").hidden = type !== "phosphorylation";
  $("ptm-charge").disabled = type !== "phosphorylation";
  $("ptm-add").disabled = !$("ptm-residue").options.length;
  $("ptm-empty").hidden = !!$("ptm-residue").options.length;
}

function render() {
  $("protein-mutations").replaceChildren(...[...mutations].map(([id, target]) =>
    removable(residueLabel(residueById.get(id)) + " → " + target, () => {
      if (!compatibleMutation(id, residueById.get(id))) return;
      mutations.delete(id); markChanged(); render();
    })));
  $("protein-ptms").replaceChildren();
  for (const r of loaded.ptms.filter(r => chains.has(r.chain))) {
    const item = document.createElement("div"); item.className = "protein-item"; item.textContent = residueLabel(r);
    $("protein-ptms").append(item);
  }
  for (const ptm of ptms.filter(p => selected(p.residue))) {
    if (ptm.type === "glycosylation") {
      const item = document.createElement("div"); item.className = "protein-item glycan-item";
      const text = document.createElement("div"), label = document.createElement("span"), sequence = document.createElement("code");
      label.textContent = `${residueLabel(residue(ptm.residue))} (${attachmentBond(residue(ptm.residue).name, attachmentAnomer(ptm.iupac || "GlcNAc", ptm.root_anomer))})`;
      sequence.textContent = (ptm.iupac || "GlcNAc").replace(/\s/g, "");
      text.append(label, sequence);
      const buttons = document.createElement("div"); buttons.className = "glycan-buttons";
      const edit = document.createElement("button"); edit.type = "button"; edit.className = "secondary compact"; edit.textContent = "Edit…";
      edit.onclick = () => editGlycan(residue(ptm.residue), residueLabel(residue(ptm.residue)), ptm.iupac || "GlcNAc", (iupac, root_anomer) => {
        ptm.iupac = iupac; ptm.root_anomer = root_anomer; markChanged(); render();
      }, ptm.root_anomer);
      const remove = document.createElement("button"); remove.type = "button"; remove.className = "remove-lipid"; remove.textContent = "×";
      remove.setAttribute("aria-label", `Remove glycan from ${label.textContent}`);
      remove.onclick = () => { ptms = ptms.filter(p => p !== ptm); markChanged(); render(); };
      buttons.append(edit, remove); item.append(text, buttons); $("protein-ptms").append(item);
      continue;
    }
    $("protein-ptms").append(removable(`${residueLabel(residue(ptm.residue))}, ${ptm.type === "phosphorylation" ? `phosphate ${ptm.charge}` : ptmTypes[ptm.type].label}`, () => {
      ptms = ptms.filter(p => p !== ptm); markChanged(); render();
    }));
  }
  $("protein-ss").replaceChildren(paged(bonds.filter(b => b.residues.every(selected)), bond => {
    const label = bond.residues.map(id => residueLabel(residue(id))).join(" – ");
    const item = checkbox(label, bond.enabled, enabled => { bond.enabled = enabled; markChanged(); render(); },
      Number.isFinite(bond.distance_nm) ? `${bond.distance_nm.toFixed(3)} nm` : "");
    item.querySelector("input").disabled = bond.residues.some(id => lipidatedCysteine(id));
    return item;
  }));
  $("protein-ready").hidden = proteinState.pending;
  $("build-protein").disabled = proteinState.busy || locked;
  $("add-ptm").disabled = !Object.keys(ptmTypes).some(type => eligiblePtmResidues(type).length);
  $("add-ss").disabled = loaded.residues.filter(r => chains.has(r.chain) && r.cysteine && !lipidatedCysteine(r.id)).length < 2;
}

export function setProteinLocked(value) {
  locked = value;
  $("edit-ligand").disabled = value || ligandBusy || !ligandLoaded;
  $("parameterize-ligand").disabled = value || ligandBusy;
  $("prepared-ligands").inert = value;
  $("ligand-viewer").inert = value;
  window.dispatchEvent(new CustomEvent("preparation-lock", { detail: value }));

  $("protein-options").disabled = value || proteinState.busy || !loaded;
}

async function checkedResponse(response) {
  if (!response.ok) { const detail = await response.json().catch(() => ({})); throw new Error(detail.error || "Protein preparation failed."); }
  return response.json();
}

function fillResidues(select, residues) {
  select.replaceChildren(...residues.map(r => {
    const option = document.createElement("option"); option.value = r.id; option.textContent = residueLabel(r); return option;
  }));
}

async function showLigand(ligand) {
  if (ligandBusy || currentLigand === ligand && ligandLoaded) return;
  const generation = ++ligandGeneration;
  if (ligandLoaded && currentLigand) {
    const previous = currentLigand;
    try { ligandEdits.set(previous.structure_url, await exportLigand()); }
    catch (error) { $("parameterize-ligand").disabled = false; throw error; }
  }
  if (generation !== ligandGeneration) return;
  ligandLoaded = false; ligandEditing = false; currentLigand = ligand; clearLigandResult(); ligandError("");
  $("edit-ligand").textContent = "Edit"; $("edit-ligand").disabled = true;
  const saved = ligandEdits.get(ligand.structure_url);
  let bytes;
  if (saved) bytes = new TextEncoder().encode(saved).buffer;
  else {
    const response = await fetch(structureUrl(ligand.structure_url));
    if (!response.ok) throw new Error("The ligand structure could not be loaded.");
    bytes = await response.arrayBuffer();
  }
  if (generation !== ligandGeneration) return;
  ligandBytes = bytes;
  ligandFrame = document.createElement("iframe");
  ligandFrame.title = residueLabel(ligand.residue); ligandFrame.src = "./viewer.html";
  ligandFrame.dataset.filename = ligand.file && !saved ? "ligand.sdf" : "ligand.cif";
  $("ligand-viewer").dataset.state = "loading";
  $("ligand-viewer").replaceChildren(ligandFrame); $("ligand-viewer").hidden = false;
}

function recordLigandEditing() {
  if (editedInViewer.has(currentLigand.structure_url)) return;
  editedInViewer.add(currentLigand.structure_url);
  ligandState.sources = null; ligandState.parameters.delete(currentLigand.structure_url);
  clearLigandResult(); notifyLigands();
}

function ligandError(message) { $("ligand-error").textContent = message; $("ligand-error").hidden = !message; }

function clearLigandResult() {
  if (ligandDownload) URL.revokeObjectURL(ligandDownload);
  ligandDownload = null; $("ligand-download").hidden = true; $("ligand-download").removeAttribute("href");
}

function exportLigand() {
  if (ligandExport) return ligandExport.promise;
  const requestId = ++ligandRequest;
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  const timer = setTimeout(() => {
    if (ligandExport?.requestId !== requestId) return;
    ligandExport = null; reject(new Error("The ligand editor did not respond. Reload the ligand and retry."));
  }, 15000);
  ligandExport = { requestId, promise, resolve, reject, timer };
  ligandFrame.contentWindow.postMessage({ type: "export-ligand", requestId }, location.origin);
  return promise;
}

async function saveLigands() {
  if (ligandBusy || locked) return;
  ligandBusy = true; ligandState.busy = true;
  setProteinLocked(true); ligandError("");
  try {
    if (ligandLoaded && currentLigand) {
      ligandFrame.contentWindow.postMessage({ type: "ligand-edit", enabled: false }, location.origin);
      ligandEdits.set(currentLigand.structure_url, await exportLigand());
    }
    const sources = [];
    for (const item of ligandState.items) {
      let cif = ligandEdits.get(item.structure_url);
      if (!editedInViewer.has(item.structure_url) && (!cif || cif === originalViewerCifs.get(item.structure_url)) && item.file) {
        sources.push({item, cif: item.file});
        continue;
      }
      if (!cif || (!editedInViewer.has(item.structure_url) && cif === originalViewerCifs.get(item.structure_url))) {
        const response = await fetch(structureUrl(item.structure_url));
        if (!response.ok) throw new Error("The ligand structure could not be loaded.");
        cif = await response.text();
      }
      sources.push({ item, cif });
    }
    // Re-exporting an unchanged ligand must not invalidate the prepared complex.
    const previous = ligandState.sources;
    if (!previous || previous.length !== sources.length || sources.some((source, i) =>
      source.item !== previous[i].item || source.cif !== previous[i].cif)) ligandState.sources = sources;
    location.hash = "#membrane";
  } catch (cause) { ligandError(cause.message); }
  finally {
    ligandBusy = false; ligandState.busy = false;
    setProteinLocked(false); notifyLigands();
  }
}

export async function buildDeferredProtein({ membrane = true, request = fetch } = {}) {
  if (!proteinState.request) return null;
  const result = await checkedResponse(await request(`${api}/build`, {
    method: "POST", headers: { "Content-Type": "application/json" }, signal: AbortSignal.timeout(630000),
    body: JSON.stringify({ ...proteinState.request, membrane }),
  }));
  result.ptmSelection = detailSelection([...loaded.ptms.map(r => r.id), ...ptms.map(p => p.residue)]);
  result.ssSelection = detailSelection(bonds.filter(b => b.enabled && b.residues.every(selected)).flatMap(b => b.residues));
  proteinState.build = result; $("protein-built").hidden = false;
  // The old preview belongs to the unmodified structure. Create the new WebGL
  // context when the Protein page is visible.
  viewerGeneration++;
  frame = null; displayedBuild = null; frameReady = false; loadedBytes = null;
  $("protein-viewer").replaceChildren();
  delete $("protein-viewer").dataset.state;
  if (location.hash === "#proteins") await preview(result.structure_url, result);
  return result;
}

export function groupedLigands() {
  return Map.groupBy(ligandState.sources || [], source => source.item.residue.name);
}

export function ligandGroupLabel(name, count) {
  return count > 1 ? `${name} ×${count}` : name;
}

export function ligandHeaders(item, cif) {
  const headers = { "Content-Type": cif instanceof Blob ? "chemical/x-mdl-sdfile" : "chemical/x-cif" };
  if (!item.file && !editedInViewer.has(item.structure_url)) headers["X-Ligand-Source"] = new URL(item.structure_url, location.href).pathname;
  return headers;
}

export async function parameterizeDeferredLigands(status) {
  const sources = ligandState.sources || [];
  const groups = groupedLigands();
  const prepared = new Map();
  for (const [name, group] of groups) {
    status(`Preparing ${ligandGroupLabel(name, group.length)}…`);
    for (const source of group) {
      const { item, cif } = source;
      const headers = buildHeaders(ligandHeaders(item, cif));
      if (headers["X-Builder-Job"]) headers["X-Builder-Step"] = `ligand-${sources.indexOf(source)}`;
      const response = await fetch(`${builderApi}/ligand/parameterize`, {
        method: "POST", headers, body: cif, signal: AbortSignal.timeout(630000),
      });
      if (!response.ok) {
        const detail = await response.json().catch(() => ({}));
        throw new Error(detail.error || `Parameterization failed (HTTP ${response.status}).`);
      }
      const archive = await response.blob(), id = response.headers.get("X-Ligand-ID");
      if (!id) throw new Error("The server did not retain the ligand parameters. Restart the updated server.");
      prepared.set(item, id); ligandState.parameters.set(item.structure_url, id);
      if (item === currentLigand) {
        clearLigandResult(); ligandDownload = URL.createObjectURL(archive);
        $("ligand-download").href = ligandDownload;
        $("ligand-download").download = `${item.residue.name}-${item.residue.id}-parameters.zip`;
        $("ligand-download").hidden = false;
      }
    }
  }
  return sources.map(({item}) => prepared.get(item));
}

function notifyLigands() {
  ligandState.pending = ligandState.items.length > 0 && !ligandState.sources;
  $("ligand-ready").hidden = !ligandState.items.length || ligandState.pending;
  window.dispatchEvent(new Event("ligands-changed"));
}

const standardWaterNames = new Set(["HOH", "WAT", "SOL", "TIP3", "TIP3P"]);
function isStandardWater(item) {
  return !item.file && standardWaterNames.has(item.residue.name) && item.residue.atoms === 3;
}

export function setLigands(items) {
  ligandGeneration++;
  if (ligandExport) { clearTimeout(ligandExport.timer); ligandExport.reject(new Error("The loaded system changed.")); ligandExport = null; }
  ligandLoaded = false; currentLigand = null; ligandBytes = null; ligandFrame = null;
  editedInViewer.clear(); originalViewerCifs.clear(); ligandEdits.clear(); ligandState.items = items; ligandState.sources = null; ligandState.parameters.clear();
  clearLigandResult(); ligandError("");
  $("ligand-viewer").replaceChildren(); $("ligand-viewer").hidden = true;
  $("parameterize-ligand").disabled = locked; $("edit-ligand").disabled = true;
  const search = $("ligand-resname-search");
  search.value = "";
  search.parentElement.hidden = !items.length;
  function renderList() {
    const query = search.value.trim().toLowerCase();
    const groups = Map.groupBy(items.filter(item => item.residue.name.toLowerCase().includes(query)), item => item.residue.name);
    function button(ligand) {
      const control = document.createElement("button"); control.type = "button"; control.className = "secondary";
      control.textContent = residueLabel(ligand.residue);
      control.onclick = () => showLigand(ligand).catch(e => ligandError(e.message));
      return control;
    }
    $("prepared-ligands").replaceChildren(paged([...groups], ([name, copies]) => {
      if (copies.every(isStandardWater)) {
        const row = document.createElement("div");
        row.textContent = `${name} (${copies.length})`;
        return row;
      }
      if (copies.length === 1) return button(copies[0]);
      const group = document.createElement("details"), summary = document.createElement("summary");
      summary.textContent = `${name} (${copies.length})`;
      group.append(summary, paged(copies, button)); return group;
    }));
  }
  search.oninput = renderList;
  renderList();
  notifyLigands();
  const first = items.find(item => !isStandardWater(item));
  if (first) showLigand(first).catch(e => ligandError(e.message));
}

export async function loadPreparedProtein(report) {
  frameReady = false;
  displayedBuild = null;
  loaded = report; chains = new Set(report?.chains.map(c => c.id) || []);
  residueById = new Map((report?.residues || []).map(r => [r.id, r]));
  residueLabelCounts = new Map();
  for (const r of report?.residues || []) residueLabelCounts.set(residueKey(r), (residueLabelCounts.get(residueKey(r)) || 0) + 1);
  mutations.clear(); ptms = []; bonds = (report?.ss_bonds || []).map(b => ({ ...b, enabled: true }));
  proteinState.build = null; $("orient-protein").checked = true;
  markChanged();
  $("protein-options").disabled = !chains.size;
  if (report && chains.size) { render(); await preview(report.structure_url); }
  else {
    viewerGeneration++; frame = null; loadedBytes = null; frameReady = false;
    $("protein-viewer").replaceChildren();
    for (const name of ["ptms", "mutations", "ss"]) $("protein-" + name).replaceChildren();
    $("protein-ready").hidden = !report;
  }
}

export function setupProteins(onChange) {
  changed = onChange;
  window.addEventListener("hashchange", () => {
    if (location.hash === "#proteins" && proteinState.build && !frame && $("protein-viewer").dataset.state !== "loading") {
      preview(proteinState.build.structure_url, proteinState.build).catch(cause => error(cause.message));
    }
  });
  $("parameterize-ligand").addEventListener("click", saveLigands);
  $("edit-ligand").addEventListener("click", () => {
    if (!ligandLoaded || ligandBusy) return;
    if (!ligandEditing) recordLigandEditing();
    ligandFrame.contentWindow.postMessage({ type: "ligand-edit", enabled: !ligandEditing }, location.origin);
  });
  $("orient-protein").addEventListener("change", markChanged);
  const mutationSites = () => {
    const terms = $("mutation-search").value.trim().toLowerCase().split(/\s+/);
    fillResidues($("mutation-residue"), loaded.residues.filter(r => r.protein && selected(r.id) && !r.coarse_grained &&
      terms.every(term => residueLabel(r).toLowerCase().includes(term))));
  };
  $("mutation-target").replaceChildren(...aminoAcids.map(name => new Option(name, name)));
  $("add-mutation").onclick = () => {
    $("mutation-search").value = ""; mutationSites(); $("mutation-picker").showModal();
  };
  $("mutation-search").oninput = mutationSites;
  $("mutation-form").onsubmit = event => {
    event.preventDefault();
    if (!$("mutation-residue").value) return;
    const id = Number($("mutation-residue").value), target = $("mutation-target").value;
    const original = residueById.get(id);
    $("mutation-picker").close();
    if (!compatibleMutation(id, mutatedResidue(original, target))) return;
    if (target === original.name) mutations.delete(id); else mutations.set(id, target);
    markChanged(); render();
  };
  $("add-ptm").addEventListener("click", () => {
    for (const option of $("ptm-type").options) option.disabled = !eligiblePtmResidues(option.value).length;
    if ($("ptm-type").selectedOptions[0].disabled) $("ptm-type").value = [...$("ptm-type").options].find(option => !option.disabled).value;
    $("ptm-search").value = "";
    updatePtmDialog();
    $("ptm-picker").showModal();
  });
  $("ptm-search").addEventListener("input", updatePtmDialog);
  $("ptm-type").addEventListener("change", updatePtmDialog);
  $("ptm-form").addEventListener("submit", event => {
    event.preventDefault();
    const type = $("ptm-type").value, residue = Number($("ptm-residue").value);
    if (!eligiblePtmResidues(type).some(r => r.id === residue)) return;
    ptms.push({ residue, type, ...(type === "glycosylation" ? { iupac: "GlcNAc", root_anomer: "b" } : {}), ...(type === "phosphorylation" ? { charge: Number($("ptm-charge").value) } : {}) });
    $("ptm-picker").close(); markChanged(); render();
  });
  $("add-ss").addEventListener("click", () => {
    const residues = loaded.residues.filter(r => chains.has(r.chain) && r.cysteine && !lipidatedCysteine(r.id));
    fillResidues($("ss-first"), residues); fillResidues($("ss-second"), residues);
    $("ss-second").selectedIndex = 1; $("ss-error").hidden = true; $("ss-picker").showModal();
  });
  $("ss-form").addEventListener("submit", event => {
    event.preventDefault(); const pair = [Number($("ss-first").value), Number($("ss-second").value)];
    if (pair[0] === pair[1] || bonds.some(b => b.enabled && b.residues.some(id => pair.includes(id)))) {
      $("ss-error").textContent = "Choose two different cysteines that are not already linked."; $("ss-error").hidden = false; return;
    }
    const candidate = loaded.ss_bonds.find(b => b.residues.every(id => pair.includes(id)));
    if (!candidate) {
      const range = loaded.disulfide_range_nm.map(value => value.toFixed(2)).join("–");
      $("ss-error").textContent = `The SG–SG distance must be ${range} nm.`; $("ss-error").hidden = false; return;
    }
    const existing = bonds.find(b => b.residues.every(id => pair.includes(id)));
    if (existing) existing.enabled = true;
    else bonds.push({ ...candidate, enabled: true });
    $("ss-picker").close(); markChanged(); render();
  });
  for (const button of document.querySelectorAll("[data-close]")) button.addEventListener("click", () => $(button.dataset.close).close());
  $("build-protein").addEventListener("click", () => {
    if (!loaded || proteinState.busy || locked) return;
    proteinState.request = chains.size ? { ...(loaded.partition_id ? { partition_id: loaded.partition_id } : { upload_id: loaded.upload_id }), chains: [...chains], ligands: [],
      ...(mutations.size ? {mutations: [...mutations].map(([residue, target]) => ({residue, target}))} : {}),
      ptms: ptms.filter(p => selected(p.residue)), ss_bonds: bonds.filter(b => b.enabled && b.residues.every(selected)).map(b => b.residues),
      orient: $("orient-protein").checked } : null;
    proteinState.pending = false; render(); changed(); location.hash = ligandState.items.length ? "#ligands" : "#membrane";
  });
  window.addEventListener("message", event => {
    if (event.origin !== location.origin) return;
    if (event.source === frame?.contentWindow) {
      if (event.data?.type === "ready" && loadedBytes) {
        frame.contentWindow.postMessage({ type: "file", mode: "protein", name: "protein.cif", bytes: loadedBytes,
          protein: displayedBuild?.protein_selection || proteinSelection(),
          proteinStyle: !displayedBuild && loaded.coarse_grained ? "vdw" : "cartoon",
          ...previewDetails(), membranePlanes: displayedBuild?.membrane_planes,
        }, location.origin, [loadedBytes]); loadedBytes = null;
      } else if (event.data?.type === "rendered") {
        frameReady = true; $("protein-viewer").dataset.state = "rendered"; updatePreview();
      } else if (event.data?.type === "invalid" || event.data?.type === "unavailable") {
        $("protein-viewer").dataset.state = "unavailable";
        error(event.data.message || "The protein viewer is unavailable.");
      }
    } else if (event.source === ligandFrame?.contentWindow) {
      if (event.data?.type === "ready" && ligandBytes) {
        ligandFrame.contentWindow.postMessage({ type: "file", mode: "ligand", name: ligandFrame.dataset.filename,
          bytes: ligandBytes }, location.origin, [ligandBytes]); ligandBytes = null;
      } else if (event.data?.type === "rendered") {
        if (event.data.initialLigandCif && !originalViewerCifs.has(currentLigand.structure_url))
          originalViewerCifs.set(currentLigand.structure_url, event.data.initialLigandCif);
        ligandLoaded = true; $("ligand-viewer").dataset.state = "rendered";
        $("parameterize-ligand").disabled = locked || ligandBusy; $("edit-ligand").disabled = locked || ligandBusy;
      } else if (event.data?.type === "ligand-export" && event.data.requestId === ligandExport?.requestId) {
        const pending = ligandExport; ligandExport = null; clearTimeout(pending.timer);
        event.data.error ? pending.reject(new Error(event.data.error)) : pending.resolve(event.data.cif);
      } else if (event.data?.type === "ligand-editing") {
        if (event.data.enabled && currentLigand) recordLigandEditing();
        ligandEditing = event.data.enabled; $("edit-ligand").textContent = ligandEditing ? "Done" : "Edit";
      } else if (event.data?.type === "ligand-changed") {
        clearLigandResult();
        if (currentLigand) {
          ligandEdits.set(currentLigand.structure_url, event.data.cif);
          ligandState.parameters.delete(currentLigand.structure_url); ligandState.sources = null; notifyLigands();
        }
      } else if (["invalid", "unavailable"].includes(event.data?.type)) {
        ligandLoaded = false; $("parameterize-ligand").disabled = locked; $("edit-ligand").disabled = true;
        $("ligand-viewer").dataset.state = "unavailable";
        ligandError(event.data.message || "The ligand viewer is unavailable.");
      }
    }
  });
}
