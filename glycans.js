import { builderApi } from "./builder-api.js";

const $ = id => document.getElementById(id);
let generation = 0, controller, timer, result, selected, site, save, history = [];
let initialized = false, attachmentText = "", initialAnomer = "b";
const definition = value => ({iupac: value.iupac, root_anomer: value.root_anomer});
export const attachmentAnomer = (iupac, anomer) => anomer || iupac.replace(/\s/g, "").match(/\(([ab])\d+-$/)?.[1] || "b";
export const attachmentBond = (site, anomer) => `${anomer === "a" ? "α" : "β"}1 → ${{ASN:"ND2",SER:"OG",THR:"OG1"}[site]}`;

export function formatIupac(text) {
  const lines = []; let line = "", depth = 0;
  const flush = () => { if (line) lines.push("  ".repeat(depth) + line); line = ""; };
  for (const character of text.replace(/\s/g, "").replace(/\([ab]\d+-$/, "")) {
    if (character === "[") { line += character; flush(); depth++; }
    else if (character === "]") { flush(); depth = Math.max(0, depth - 1); line = "]"; flush(); }
    else line += character;
  }
  flush(); return lines.join("\n");
}

function error(message = "") {
  $("glycan-error").textContent = message; $("glycan-error").hidden = !message;
}
function controls(disabled) {
  $("glycan-tree-controls").disabled = disabled;
  $("glycan-apply").disabled = disabled;
  $("glycan-format").disabled = disabled;
  $("glycan-tree").inert = disabled;
}
function options(select, values) {
  select.replaceChildren(...values.map(value => new Option(String(value), String(value))));
}
function availablePositions(parent, except) {
  const occupied = new Set(result.nodes.filter(n => n.parent === parent && n.id !== except).map(n => n.position));
  return (result.acceptors[parent] || []).filter(p => !occupied.has(p));
}
function addDonor() {
  options($("glycan-add-donor"), [result.donors[$("glycan-add-sugar").value]]);
  $("glycan-add-donor").disabled = $("glycan-add-donor").options.length <= 1;
}
function selectNode(id) {
  selected = id;
  const node = result.nodes.find(n => n.id === id);
  for (const marker of $("glycan-tree").querySelectorAll(".glycan-selection-outline")) marker.remove();
  for (const shape of $("glycan-tree").querySelectorAll("[data-node-index]")) {
    const active = Number(shape.dataset.nodeIndex) === selected;
    shape.classList.toggle("glycan-selected", active); shape.setAttribute("aria-pressed", String(active));
    if (active) {
      const box = shape.getBBox(), marker = document.createElementNS("http://www.w3.org/2000/svg", "rect");
      for (const [key, value] of Object.entries({x:box.x-7, y:box.y-7, width:box.width+14, height:box.height+14, rx:4})) marker.setAttribute(key, value);
      marker.setAttribute("class", "glycan-selection-outline");
      marker.setAttribute("aria-hidden", "true"); shape.prepend(marker);
    }
  }
  $("glycan-selected-label").textContent = node.sugar;
  $("glycan-node-sugar").value = node.sugar; $("glycan-node-anomer").value = node.anomer;
  options($("glycan-node-donor"), [result.donors[node.sugar]]);
  $("glycan-node-donor").disabled = $("glycan-node-donor").options.length <= 1;
  $("glycan-link-field").hidden = node.parent === null;
  options($("glycan-node-position"), node.parent === null ? [] : availablePositions(node.parent, node.id));
  $("glycan-node-position").value = String(node.position ?? 4);
  $("glycan-remove").disabled = node.parent === null;
  const available = availablePositions(id);
  options($("glycan-add-position"), available);
  if (available.includes(4)) $("glycan-add-position").value = "4";
  $("glycan-add").disabled = !available.length;
  addDonor();
}
function render() {
  $("glycan-attachment").textContent = `${attachmentText} (${attachmentBond(site, result.root_anomer)})`;
  const parsed = new DOMParser().parseFromString(result.svg, "image/svg+xml");
  const svg = parsed.documentElement;
  // Rendering comes from the server's validated sugar graph, never from text HTML.
  for (const element of svg.querySelectorAll("script, foreignObject")) element.remove();
  for (const element of svg.querySelectorAll("*")) {
    for (const attr of [...element.attributes]) if (/^on/i.test(attr.name) || /href$/i.test(attr.name)) element.removeAttribute(attr.name);
  }
  $("glycan-tree").replaceChildren(document.importNode(svg, true));
  options($("glycan-node-sugar"), result.sugars);
  const previous = $("glycan-add-sugar").value;
  options($("glycan-add-sugar"), result.sugars);
  $("glycan-add-sugar").value = result.sugars.includes(previous) ? previous : "GlcNAc";
  for (const edge of $("glycan-tree").querySelectorAll("[data-edge-index]")) {
    const label = edge.querySelector("text.link");
    if (label) label.textContent = result.linkage_labels[edge.dataset.edgeIndex];
  }
  for (const shape of $("glycan-tree").querySelectorAll("[data-node-index]")) {
    const id = Number(shape.dataset.nodeIndex), node = result.nodes.find(n => n.id === id);
    shape.setAttribute("tabindex", "0"); shape.setAttribute("role", "button");
    shape.setAttribute("aria-label", `${node.sugar}, ${node.anomer === "a" ? "alpha" : "beta"}${node.position ? `, linkage ${result.donors[node.sugar]}–${node.position}` : ", protein attachment"}`);
    shape.addEventListener("click", () => selectNode(id));
    shape.addEventListener("keydown", event => { if (["Enter", " "].includes(event.key)) { event.preventDefault(); selectNode(id); } });
  }
  selectNode(result.nodes.some(n => n.id === selected) ? selected : result.nodes.find(n => n.parent === null).id);
  $("glycan-undo").disabled = !history.length;
}
async function update(payload, {format = true, remember = true} = {}) {
  if (payload.iupac !== undefined && payload.root_anomer === undefined) payload = {...payload, root_anomer: result?.root_anomer || initialAnomer};
  clearTimeout(timer); controller?.abort(); controller = new AbortController();
  const version = ++generation;
  controls(true); error(); $("glycan-status").hidden = false;
  try {
    const response = await fetch(`${builderApi}/glycan`, {
      method: "POST", headers: {"Content-Type": "application/json"},
      body: JSON.stringify({site, ...payload}), signal: controller.signal,
    });
    const next = await response.json();
    if (version !== generation) return;
    if (!response.ok) throw new Error(next.error || "The glycan could not be parsed.");
    if (remember && result && (result.iupac !== next.iupac || result.root_anomer !== next.root_anomer)) history.push(definition(result));
    selected = payload.nodes ? next.id_map?.[selected] ?? null : null;
    result = next;
    if (format) $("glycan-iupac").value = next.formatted_iupac;
    render(); controls(false);
  } catch (cause) {
    if (version !== generation || cause.name === "AbortError") return;
    if (payload.nodes && result) { render(); controls(false); }
    error(cause.message);
    // Keep the last valid tree visible; saving invalid text stays disabled.
    $("glycan-undo").disabled = !result;
  } finally {
    if (version === generation) $("glycan-status").hidden = true;
  }
}
function changeNode() {
  const nodes = structuredClone(result.nodes), node = nodes.find(n => n.id === selected);
  node.sugar = $("glycan-node-sugar").value; node.anomer = $("glycan-node-anomer").value;
  if (node.parent !== null) node.position = Number($("glycan-node-position").value);
  update({nodes});
}
function setup() {
  if (initialized) return; initialized = true;
  $("glycan-iupac").addEventListener("input", () => {
    controller?.abort(); generation++; controls(true); error(); clearTimeout(timer);
    timer = setTimeout(() => update({iupac: $("glycan-iupac").value}, {format: false}), 300);
  });
  $("glycan-format").onclick = () => { $("glycan-iupac").value = result.formatted_iupac; };
  for (const id of ["glycan-node-sugar", "glycan-node-anomer", "glycan-node-position"]) $(id).onchange = changeNode;
  $("glycan-add-sugar").onchange = addDonor;
  $("glycan-add").onclick = () => {
    const nodes = structuredClone(result.nodes);
    const id = Math.max(...nodes.map(n => n.id)) + 1;
    nodes.push({id, sugar: $("glycan-add-sugar").value, anomer: $("glycan-add-anomer").value, parent: selected, position: Number($("glycan-add-position").value)});
    update({nodes});
  };
  $("glycan-remove").onclick = () => {
    const remove = new Set([selected]);
    let previous;
    do { previous = remove.size; for (const n of result.nodes) if (remove.has(n.parent)) remove.add(n.id); } while (previous !== remove.size);
    selected = result.nodes.find(n => n.id === selected).parent;
    update({nodes: result.nodes.filter(n => !remove.has(n.id))});
  };
  $("glycan-undo").onclick = () => update($("glycan-apply").disabled ? definition(result) : history.pop() || definition(result), {remember: false});
  $("glycan-apply").onclick = () => { save(result.iupac, result.root_anomer); $("glycan-picker").close(); };
  $("glycan-cancel").onclick = () => $("glycan-picker").close();
  $("glycan-picker").addEventListener("close", () => {
    if ($("glycan-picker").open) return;
    clearTimeout(timer); generation++; controller?.abort();
  });
}

export function editGlycan(residue, label, iupac, onSave, rootAnomer) {
  attachmentText = label; initialAnomer = attachmentAnomer(iupac, rootAnomer);
  setup(); site = residue.name; save = onSave; result = null; selected = null; history = [];
  $("glycan-attachment").textContent = label;
  $("glycan-iupac").value = formatIupac(iupac);
  $("glycan-tree").replaceChildren(); $("glycan-undo").disabled = true;
  $("glycan-picker").showModal(); update({iupac}, {remember: false});
}
