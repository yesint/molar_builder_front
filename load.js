import { builderApi, structureUrl } from "./builder-api.js";
import { loadPreparedProtein, setLigands, formatResidueLabel } from "./protein.js";

const $ = id => document.getElementById(id);
const sources = [];
const subsetUrls = [];
let nextSourceId = 0, selectionVersion = 0;
let frame = null, locked = false, nextPage = "#membrane";
const api = builderApi;
export const loadState = { done: false, busy: false, hasFile: false, hasStructure: false };

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
  if (!loadState.done) return;
  loadState.done = false; $("load-ready").hidden = true;
  loadPreparedProtein(null); setLigands([]);
  for (const url of subsetUrls.splice(0)) URL.revokeObjectURL(url);
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
function union(parts) {
  const expressions = [...new Set(parts.filter(Boolean))];
  return expressions.length < 2 ? expressions[0] || "" : expressions.map(s => `(${s})`).join(" or ");
}
function residueSelection(ids) {
  const ranges = [];
  for (let i = 0; i < ids.length; i++) {
    const first = ids[i];
    while (ids[i + 1] === ids[i] + 1) i++;
    ranges.push(first === ids[i] ? first : `${first}:${ids[i]}`);
  }
  return ranges.length ? "resindex " + ranges.join(" ") : "";
}
function selectionButton(symbol, label) {
  const button = document.createElement("button");
  button.type = "button"; button.className = "secondary compact"; button.textContent = symbol;
  button.setAttribute("aria-label", label);
  return button;
}
function setMode(component, custom) {
  const list = component === "protein" ? "chains" : "residues";
  for (const [mode, active] of [[list, !custom], ["custom", custom]]) {
    const tab = $(component + "-" + mode + "-tab");
    tab.setAttribute("aria-selected", String(active)); tab.tabIndex = active ? 0 : -1;
    $(component + "-" + mode + "-panel").hidden = !active;
  }
}
async function checked(response) {
  const result = await response.json(); if (!response.ok) throw new Error(result.error || `Load failed (HTTP ${response.status}).`); return result;
}

function newModel(report, record, initiallySelected) {
  const model = {report, record, custom: {protein: [], ligand: []}, drafts: {protein: "", ligand: ""},
    choices: {protein: [], ligand: []}, chosen: {protein: new Set(), ligand: new Set()}};
  function add(role, label, keys, selected, residues) {
    model.choices[role].push({label, keys, residues});
    if (initiallySelected && selected) for (const key of keys) model.chosen[role].add(key);
  }
  if (record) {
    add("ligand", record.label, ["all"], true);
    return model;
  }
  for (const chain of report.chains) add("protein", chain.id.trim() ? "Chain " + chain.id : "No chain ID", [chain.id], true);
  const unknown = report.other || [];
  const autoUnknown = unknown.length === 1 && !report.chains.length && !report.ligands.length && !report.environment?.length;
  for (const [residues, selected] of [[report.ligands, true], [unknown, autoUnknown]]) {
    for (const [name, group] of Map.groupBy(residues, r => r.name)) {
      add("ligand", group.length === 1 ? formatResidueLabel(group[0], true) : `${name} (${group.length})`,
        group.map(r => r.id), selected, group.length > 1 ? group : null);
    }
  }
  for (const item of report.environment || []) {
    add("ligand", `${item.name}, ${item.category.replaceAll("_", " ")} (${item.residues})`, [item.name], false);
  }
  return model;
}
function expression(model, role) {
  const {report} = model, chosen = model.chosen[role], parts = [];
  if (chosen.size) {
    if (!report) parts.push("all");
    else if (role === "protein" && chosen.size === report.chains.length && report.protein_selection) parts.push(report.protein_selection);
    else {
      const ids = report.residues.filter(r => role === "protein" ? r.protein && chosen.has(r.chain) : chosen.has(r.id))
        .map(r => r.preview_index ?? r.id);
      parts.push(residueSelection(ids));
      if (role === "ligand") {
        const names = (report.environment || []).filter(e => chosen.has(e.name)).map(e => e.name);
        if (names.length) {
          const except = residueSelection(report.residues.filter(r => chosen.has(r.name)).map(r => r.preview_index ?? r.id));
          const group = "resname " + names.join(" ");
          parts.push(except ? `((${group}) and not (${except}))` : group);
        }
      }
    }
  }
  return union([...parts, ...model.custom[role]]);
}
function modelLabel(model) {
  return (model.record ? "mol " : "model ") + model.number;
}
function checkbox(label, read, change) {
  const row = document.createElement("label"), box = document.createElement("input");
  row.className = "protein-check"; box.type = "checkbox";
  box.refresh = () => {
    const [selected, total] = read();
    box.checked = total > 0 && selected === total; box.indeterminate = selected > 0 && selected < total;
    box.disabled = !total;
  };
  box.refresh();
  box.onchange = () => change(box.checked);
  row.append(box, document.createTextNode(label)); return row;
}
function chooseGroup(source, role) {
  const group = document.createElement("div"), title = document.createElement("h4");
  group.className = "component-source"; title.textContent = source.filename; group.append(title);
  const total = source.models.reduce((sum, m) => sum + m.choices[role].reduce((n, c) => n + c.keys.length, 0), 0);
  const update = () => {
    for (const box of group.querySelectorAll("input")) box.refresh();
    invalidate(); updateViewer();
  };
  const all = checkbox("All", () => [source.models.reduce((n, m) => n + m.chosen[role].size, 0), total], checked => {
    for (const model of source.models) {
      const chosen = model.chosen[role]; chosen.clear();
      if (checked) for (const choice of model.choices[role]) for (const key of choice.keys) chosen.add(key);
    }
    update();
  });
  group.append(all);
  function row(model, label, keys) {
    const chosen = model.chosen[role];
    return checkbox(label, () => {
      if (!chosen.size) return [0, keys.length];
      let count = 0; for (const key of keys) if (chosen.has(key)) count++;
      return [count, keys.length];
    }, checked => {
      for (const key of keys) checked ? chosen.add(key) : chosen.delete(key);
      update();
    });
  }
  const choices = source.models.flatMap(model => model.choices[role].map(choice => ({model, choice})));
  group.append(paged(choices, ({model, choice}) => {
    const prefix = source.models.length > 1 && model.report ? `${modelLabel(model)} · ` : "";
    const control = row(model, prefix + choice.label, choice.keys);
    if (!choice.residues) return control;
    const wrap = document.createElement("div"), details = document.createElement("details");
    const summary = document.createElement("summary"), children = document.createElement("div");
    summary.textContent = "Residues";
    details.ontoggle = () => children.replaceChildren(...(details.open ? [paged(choice.residues,
      r => row(model, formatResidueLabel(r, true), [r.id]))] : []));
    details.append(summary, children); wrap.append(control, details); return wrap;
  }));
  return group;
}
function selectionEditor(source, role) {
  const group = document.createElement("div"), row = document.createElement("div"), list = document.createElement("ul");
  const label = document.createElement("label"), input = document.createElement("input");
  const add = selectionButton("+", `Add ${role} selection for ${source.filename}`);
  group.className = "custom-source"; row.className = "selection-entry"; label.className = "selection-source";
  input.type = "text"; input.spellcheck = false; input.autocomplete = "off"; input.id = `${role}-selection-${source.id}`;
  label.htmlFor = input.id; label.textContent = source.filename; row.append(label);
  let current = source.models[source.active[role]];
  function showDraft() { input.value = current.drafts[role]; add.disabled = !input.value.trim(); }
  if (source.models.length > 1) {
    const select = document.createElement("select");
    select.setAttribute("aria-label", `Model for ${source.filename} ${role} selection`);
    for (const model of source.models) select.add(new Option(modelLabel(model), model.number));
    select.selectedIndex = source.active[role];
    select.onchange = () => { source.active[role] = select.selectedIndex; current = source.models[select.selectedIndex]; showDraft(); };
    row.classList.add("with-model"); row.append(select);
  }
  row.append(input, add); list.className = "saved-selections"; group.append(row, list);
  function renderEntries() {
    list.replaceChildren(...source.models.flatMap(model => model.custom[role].map((expression, i) => {
      const li = document.createElement("li");
      const remove = selectionButton("×", `Remove ${role} selection ${expression} from ${source.filename}, ${modelLabel(model)}`);
      remove.onclick = () => { model.custom[role].splice(i, 1); renderEntries(); invalidate(); updateViewer(); input.focus(); };
      if (source.models.length > 1) {
        const prefix = document.createElement("span"); prefix.textContent = modelLabel(model); li.append(prefix);
      }
      const field = document.createElement("input"); field.type = "text"; field.value = expression;
      field.spellcheck = false; field.setAttribute("aria-label", `${source.filename}, ${modelLabel(model)}, saved ${role} selection ${i + 1}`);
      field.onchange = () => {
        const value = field.value.trim();
        if (!value) { field.value = model.custom[role][i]; return; }
        model.custom[role][i] = field.value = value; invalidate(); updateViewer();
      };
      field.onkeydown = event => { if (event.key === "Enter") { event.preventDefault(); field.blur(); } };
      li.append(field, remove); return li;
    })));
  }
  input.oninput = () => { current.drafts[role] = input.value; add.disabled = !input.value.trim(); };
  add.onclick = () => {
    const expression = input.value.trim(); if (!expression) return;
    if (!current.custom[role].includes(expression)) current.custom[role].push(expression);
    current.drafts[role] = ""; showDraft(); renderEntries(); invalidate(); updateViewer(); input.focus();
  };
  input.onkeydown = event => { if (event.key === "Enter") { event.preventDefault(); add.click(); } };
  showDraft(); renderEntries(); return group;
}

function primaryModel() { return sources.find(source => source.models[0].report)?.models[0]; }
function viewerSelection() {
  const primary = primaryModel();
  const externalLigands = sources.flatMap(source => source.models.filter(m => m !== primary).map(m => ({
    key: m.record?.structure_url || structureUrl(m.report.structure_url), format: m.record ? "sdf" : "cif",
    style: m.report ? (m.report.coarse_grained ? "vdw" : "cartoon") : "ballstick",
    protein: expression(m, "protein"), ligands: expression(m, "ligand"),
  })));
  return { protein: primary ? expression(primary, "protein") : "", ligands: primary ? expression(primary, "ligand") : "", externalLigands };
}
function updateViewer() {
  error(); selectionVersion++;
  frame?.contentWindow.postMessage({type: "protein-selection", requestId: selectionVersion, ...viewerSelection()}, location.origin);
}
function refreshFiles() {
  loadState.hasFile = sources.length > 0; loadState.hasStructure = Boolean(primaryModel());
  for (const [picker, host] of [["structure-file", "structure-filename"], ["ligand-files", "ligand-filenames"]]) {
    $(host).replaceChildren(...sources.filter(source => source.picker === picker).map(source => {
      const name = document.createElement("span"); name.className = "component-filename"; name.append(source.filename);
      const remove = selectionButton("×", "Remove " + source.filename);
      remove.onclick = () => {
        if (locked || loadState.busy) return;
        sources.splice(sources.indexOf(source), 1); refreshFiles();
        for (const m of source.models) if (m.record) URL.revokeObjectURL(m.record.structure_url);
        busy(false);
      };
      name.append(remove); return name;
    }));
  }
  for (const [role, id] of [["protein", "load-chains"], ["ligand", "load-ligands"]]) {
    $(id).replaceChildren(...sources.filter(s => s.models.some(m => m.choices[role].length)).map(s => chooseGroup(s, role)));
    $(role + "-custom-panel").replaceChildren(...sources.map(s => selectionEditor(s, role)));
  }
  for (const id of ["load-options", "load-actions", "load-viewer"]) $(id).hidden = !loadState.hasFile;
  invalidate();
  frame = null; $("load-viewer").replaceChildren(); delete $("load-viewer").dataset.state;
  if (loadState.hasFile) {
    frame = document.createElement("iframe"); frame.title = "Loaded system"; frame.src = "./viewer.html"; $("load-viewer").append(frame);
  }
}
async function readModels(file, picker) {
  const extension = file.name.split(".").pop().toLowerCase();
  if (["sdf", "sd"].includes(extension)) {
    const bytes = new Uint8Array(await file.arrayBuffer()), text = new TextDecoder("latin1").decode(bytes);
    const ends = [...text.matchAll(/^\$\$\$\$[^\S\r\n]*(?:\r?\n|$)/gm)].map(m => m.index + m[0].length);
    if (text.slice(ends.at(-1) || 0).trim()) ends.push(bytes.length);
    if (!ends.length) throw new Error(file.name + " contains no SDF records.");
    let start = 0;
    return ends.map((end, i) => {
      if (!/^M  END\s*$/m.test(text.slice(start, end))) throw new Error(file.name + " contains an invalid molfile.");
      const blob = ends.length === 1 ? file : file.slice(start, end); start = end;
      const record = {label: `mol ${i + 1}`, origin: file.name, file: blob,
        residue: {name: `${file.name} | mol ${i + 1}`, number: "", chain: ""}};
      return newModel(null, record, true);
    });
  }
  if (!["pdb","cif","mmcif"].includes(extension)) throw new Error("Choose a PDB, mmCIF or SDF file.");
  const report = await checked(await fetch(`${api}/load/structure`, { method:"POST", headers: {
    "Content-Type":"application/octet-stream", "X-Structure-Format":extension }, body:file, signal:AbortSignal.timeout(630000) }));
  return [report, ...(report.additional_models || [])].map((r, i) => {
    const model = newModel(r, null, i === 0);
    if (picker === "ligand-files") model.chosen.protein.clear();
    return model;
  });
}
async function partitionInputs() {
  const structures = [], custom = [], ordered = [];
  for (const source of sources) for (const model of source.models) {
    const protein_selection = expression(model, "protein"), ligand_selection = expression(model, "ligand");
    if (!protein_selection && !ligand_selection) continue;
    if (model.report) {
      const origin = source.models.length > 1 ? `${source.filename} | ${modelLabel(model)}` : source.filename;
      ordered.push({structure: structures.length, origin});
      structures.push({upload_id:model.report.upload_id, protein_selection, ligand_selection});
    } else if (model.custom.protein.length || model.custom.ligand.length) {
      ordered.push({custom: custom.length, model});
      custom.push({name: `${source.filename} | ${modelLabel(model)}`, sdf: await model.record.file.text(), protein_selection, ligand_selection});
    } else ordered.push({model});
  }
  let result = {protein:null, ligands:[]};
  const single = structures.length === 1 && !custom.length;
  if (structures.length || custom.length) result = await checked(await fetch(`${api}/load/partition`, {
    method:"POST", headers:{"Content-Type":"application/json"}, body:JSON.stringify(single ? structures[0] : {structures, sdf_selections:custom}),
  }));
  const ligands = [], byStructure = Map.groupBy(result.ligands, item => item.source ?? 0);
  const subsets = new Map((result.sdf_ligands || []).map(item => [item.source, item.cif]));
  for (const item of ordered) {
    if ("structure" in item) {
      for (const ligand of byStructure.get(item.structure) || []) ligands.push({...ligand, origin: item.origin});
      continue;
    }
    if ("custom" in item && !subsets.has(item.custom)) continue;
    const cif = subsets.get(item.custom);
    if (cif) {
      const url = URL.createObjectURL(new Blob([cif], {type:"chemical/x-cif"})); subsetUrls.push(url);
      ligands.push({...item.model.record, file:null, structure_url:url});
    } else ligands.push(item.model.record);
  }
  return {protein:result.protein, ligands};
}

export function setupLoad() {
  for (const role of ["protein", "ligand"]) {
    const list = role === "protein" ? "chains" : "residues";
    for (const mode of [list, "custom"]) {
      const tab = $(role + "-" + mode + "-tab");
      tab.onclick = () => setMode(role, mode === "custom");
      tab.onkeydown = event => {
        if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
        event.preventDefault(); const custom = event.key === "End" || (event.key !== "Home" && mode !== "custom");
        setMode(role, custom); $(role + "-" + (custom ? "custom" : list) + "-tab").focus();
      };
    }
  }
  for (const [button, picker] of [["load-structure","structure-file"],["add-ligand-files","ligand-files"]]) {
    $(button).onclick = () => $(picker).click();
    $(picker).onchange = async () => {
      busy(true, "Reading components…"); error();
      try {
        const batch = [];
        for (const file of $(picker).files) {
          const models = await readModels(file, picker);
          models.forEach((model, i) => { model.number = i + 1; });
          batch.push({id:nextSourceId++, filename:file.name, picker, models, active:{protein:0,ligand:0}});
        }
        for (const source of batch) for (const m of source.models) if (m.record) m.record.structure_url = URL.createObjectURL(m.record.file);
        sources.push(...batch); refreshFiles();
      } catch (cause) { error(cause.message); }
      finally { busy(false); $(picker).value = ""; }
    };
  }
  $("partition-system").onclick = async () => {
    if (!sources.length || locked || loadState.busy) return;
    if (loadState.done) { location.hash = nextPage; return; }
    busy(true, "Partitioning system…"); error();
    try {
      const {protein, ligands} = await partitionInputs();
      await loadPreparedProtein(protein); setLigands(ligands);
      loadState.done = true; $("load-ready").hidden = false;
      loadState.hasStructure = Boolean(primaryModel() || protein?.chains.length);
      nextPage = protein?.chains.length ? "#proteins" : ligands.length ? "#ligands" : "#membrane";
      location.hash = nextPage;
    } catch (cause) { error(cause.message); }
    finally { busy(false); }
  };
  window.addEventListener("preparation-lock", event => { locked = event.detail; busy(loadState.busy); });
  window.addEventListener("message", async event => {
    if (event.origin !== location.origin || event.source !== frame?.contentWindow) return;
    if (event.data?.type === "ready") {
      const target = frame, primary = primaryModel();
      try {
        let bytes = null;
        if (primary) {
          const response = await fetch(structureUrl(primary.report.structure_url));
          if (!response.ok) throw new Error("The loaded structure is no longer available.");
          bytes = await response.arrayBuffer();
        }
        if (frame !== target) return;
        frame.contentWindow.postMessage({type:"file", mode:"protein", name:"loaded.cif", bytes, ...viewerSelection(),
          proteinStyle:primary?.report.coarse_grained ? "vdw" : "cartoon",
          ligandStyle:primary?.report.chains.length ? "licorice" : "ballstick"}, location.origin, bytes ? [bytes] : []);
      } catch (cause) { if (frame === target) error(cause.message); }
    } else if (event.data?.type === "rendered") {
      $("load-viewer").dataset.state = "rendered"; updateViewer();
    } else if (event.data?.type === "selection-error" && event.data.requestId === selectionVersion) error(event.data.message);
    else if (["invalid", "unavailable"].includes(event.data?.type)) error(event.data.message || "The structure viewer is unavailable.");
  });
}
