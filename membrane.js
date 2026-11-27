import { buildHeaders, checkoutBuild, receiveCheckoutReturn, sha256 } from "./builder-checkout.js";
import { ligandHeaders } from "./protein.js";
import { builderApi, publicBuilder } from "./builder-api.js";
import { normalizeComposition, lipidCounts, monolayerAreas, areaMismatch, balanceLipid, membraneCellSize } from "./membrane-composition.js";
import { proteinState, ligandState, setupProteins, buildDeferredProtein, parameterizeDeferredLigands, groupedLigands, ligandGroupLabel } from "./protein.js";

import { loadState, setupLoad } from "./load.js";
import { invalidateAssembly, setAssemblyMembrane, setupAssembly } from "./assembly.js";

let complex = null, assemblyBusy = false, membraneRequest = null, preparationKey = [];
let balanceSpecies = null;
let builtMembrane = null;
const componentsReady = () => !loadState.hasFile || loadState.done;
const preparationPending = () => !componentsReady() || loadState.busy || proteinState.pending || proteinState.busy || ligandState.pending || ligandState.busy || assemblyBusy;

const hasSolute = () => proteinState.pending || !!proteinState.request || ligandState.items.length > 0;
const batchReady = () => ready && !preparationPending() && (!$("add-membrane").checked ? hasSolute() : !!membraneRequest);

const $ = id => document.getElementById(id);
$("open-about").addEventListener("click", () => $("about-builder").showModal());
for (const button of document.querySelectorAll(".stage-heading .help-button")) {
  button.addEventListener("click", () => {
    $("stage-help-heading").textContent = button.closest(".stage-heading").querySelector("h2").textContent;
    $("stage-help-text").textContent = button.dataset.help;
    $("stage-help").showModal();
  });
}
receiveCheckoutReturn($("checkout-return"));
let catalogue = [];
let selectedFamily = null;
let selectedSubfamily = null;
let rows = [];
let normalizedRows = null;
let balanceAdjustment = null;
let baseCounts = [];
// Preserve the builder's coordinate convention: inner = lower (z < 0), outer = upper (z > 0).
const sides = [{ key: "lower", name: "inner" }, { key: "upper", name: "outer" }];
let proteinAreas = { upper: 0, lower: 0 };
let busy = false;
let ready = false;
let testCheckout = false, paypalMode = null;
let resultURL = null;
let unsaved = false;
let timeoutSeconds = 600;
const api = `${builderApi}/membrane`;
const format = value => Number.isFinite(value) ? `${Number(value.toFixed(3))}%` : "—";

function showError(message) {
  $("composition-error").textContent = message;
  $("composition-error").hidden = !message;
}

// Each step records configuration. Only Assembly executes the deferred work.
function syncMembraneStep() {
  const membrane = $("add-membrane").checked;
  $("membrane-composition").hidden = !membrane; $("no-membrane-note").hidden = membrane;
  $("no-membrane-actions").hidden = membrane;
  $("continue-no-membrane").disabled = !loadState.done || loadState.busy || !hasSolute() || assemblyBusy;
  setAssemblyMembrane(membrane);
  $("membrane-ready").hidden = membrane ? !membraneRequest : !loadState.done || !hasSolute();
  invalidateAssembly();
}
function invalidate(resetBalance = true, keepConfiguration = false) {
  if (resetBalance) { balanceAdjustment = null; balanceSpecies = null; }
  if (!keepConfiguration) membraneRequest = null;
  builtMembrane = null; syncMembraneStep();
  normalizedRows = null;
  $("composition-controls").hidden = false;
  $("computation").hidden = true;
  $("built-membrane").hidden = true;
  clearViewer();
  $("save-membrane").hidden = true;
  if (resultURL) URL.revokeObjectURL(resultURL);
  resultURL = null;
  unsaved = false;
  showError("");
}

function refreshComposition() {
  let normalized = [];
  let counts = [];
  const estimated = hasSolute() && !complex;
  baseCounts = [];
  $("resolved-cell").textContent = "";
  normalizedRows = null;
  showError("");
  try {
    normalized = normalizeComposition(rows, $("per-leaflet").checked);
    const cell = $("cell-size");
    const minimum = Number(cell.min);
    if (!cell.checkValidity()) {
      const limit = cell.max ? `between ${minimum} and ${cell.max} nm` : `at least ${minimum.toFixed(1)} nm`;
      throw new Error(`Enter a cell length ${limit}, in steps of 0.1 nm.`);
    }
    if (!$("xy-margin").checkValidity()) throw new Error("Enter a finite, nonnegative XY margin.");
    // Before preparation, preview the entered cell without subtracting an unknown
    // protein area. Assembly replaces these estimates with the prepared geometry.
    const size = membraneCellSize(cell.valueAsNumber, $("xy-margin").valueAsNumber, complex);
    counts = lipidCounts(rows, catalogue, size, $("per-leaflet").checked, proteinAreas);
    baseCounts = counts.map(row => ({ ...row }));
    if (balanceAdjustment) counts.find(row => row.id === balanceAdjustment.id)[balanceAdjustment.side] += balanceAdjustment.delta;
    for (const { key, name } of sides) {
      if (!counts.some(row => row[key] > 0)) throw new Error(`The cell is too small for any lipid in the ${name} leaflet.`);
    }
    $("resolved-cell").textContent = complex ? `Cell length after orientation: ${size.toFixed(1)} nm` : "";
    normalizedRows = normalized;
  } catch (error) { if (rows.length) showError(error.message); }
  $("composition-estimate").hidden = !estimated || !normalizedRows;
  for (const row of rows) {
    for (const { key, name } of sides) {
      const percentage = normalized.find(item => item.id === row.id)?.[key];
      const renormalized = $(`${row.id}-${name}-normalized`);
      renormalized.textContent = format(percentage);
      renormalized.setAttribute("aria-label", `${row.id} ${name} normalized percentage: ${format(percentage)}`);
      const output = $(`${row.id}-${name}-count`);
      const value = counts.find(item => item.id === row.id)?.[key];
      output.textContent = `(${value ?? "—"})`;
      output.setAttribute("aria-label", value === undefined ? "Lipid count unavailable" : `${value} lipids${estimated ? " (estimated)" : ""}`);
      const adjustment = $(`${row.id}-${name}-adjustment`);
      const delta = balanceAdjustment?.id === row.id && balanceAdjustment.side === key ? balanceAdjustment.delta : 0;
      adjustment.textContent = delta ? `${delta > 0 ? "+" : ""}${delta}` : "";
      adjustment.hidden = !delta;
    }
  }
  for (const { key, name } of sides) {
    $(name + "-total").textContent = normalized.length ? format(normalized.reduce((sum, row) => sum + row[key], 0)) : "—";
    $(name + "-total-count").textContent = counts.length ? `(${counts.reduce((sum, row) => sum + row[key], 0)})` : "(—)";
  }
  $("combined-total").textContent = `Normalized membrane total: ${normalized.length ? format(normalized.reduce((sum, row) => sum + row.upper + row.lower, 0)) : "—"}`;
  $("combined-total").hidden = $("per-leaflet").checked;
  const lipidAreas = counts.length && normalizedRows ? monolayerAreas(counts, catalogue) : null;
  const areas = counts.length && normalizedRows ? monolayerAreas(counts, catalogue, proteinAreas) : null;
  for (const { key, name } of sides) {
    const other = key === "upper" ? "lower" : "upper";
    const output = $(name + "-area");
    output.textContent = lipidAreas ? lipidAreas[key].toFixed(3) : "—";
    if (areas && areas[key] > areas[other]) {
      const excess = document.createElement("span");
      excess.className = "area-excess";
      excess.textContent = `(+${Number(areaMismatch(areas).toFixed(2))}%)`;
      output.append(excess);
    }
  }
  $("balance-controls").hidden = !areas || areaMismatch(areas) <= 5;

  const selected = $("balancing-lipid").value;
  $("balancing-lipid").replaceChildren();
  for (const row of (counts.length ? counts : rows).filter(row => row.upper + row.lower > 0)) {
    const option = document.createElement("option");
    option.value = row.id;
    option.textContent = catalogue.find(lipid => lipid.id === row.id).name;
    $("balancing-lipid").append(option);
  }
  if ([...$("balancing-lipid").options].some(option => option.value === selected)) $("balancing-lipid").value = selected;
  $("balance").disabled = !normalizedRows || !!balanceSpecies || !$("balancing-lipid").value;
  updateBuildButton();
  renderCharts(normalizedRows ? counts : []);
}

// molar_vis PALETTE from crates/molar_vis_core/src/color.rs at the bundled
// revision 116bd73343c805b1269ebc78b30a0590a44c6e88 (solid-color presets).
const lipidPalette = [
  "#e63232", "#ff8c00", "#f0dc28", "#3cc850", "#3cc8d2", "#3c6ef0",
  "#9650dc", "#dc3cc8", "#f082b4", "#1e9696", "#aadc28", "#283c96",
  "#f5f5f5", "#c8c8c8", "#8c8c8c", "#505050", "#191919", "#965a32",
];
function speciesColor(id) {
  return lipidPalette[rows.findIndex(lipid => lipid.id === id) % lipidPalette.length];
}

function renderCharts(counts) {
  const svgElement = (tag, attributes) => {
    const element = document.createElementNS("http://www.w3.org/2000/svg", tag);
    for (const [key, value] of Object.entries(attributes)) element.setAttribute(key, value);
    return element;
  };
  for (const { key, name } of sides) {
    const chart = $(name + "-pie");
    const legend = $(name + "-legend");
    chart.replaceChildren();
    legend.replaceChildren();
    const entries = counts.filter(row => row[key] > 0);
    const total = entries.reduce((sum, row) => sum + row[key], 0);
    chart.dataset.total = total;
    chart.setAttribute("aria-label", `${name === "inner" ? "Inner" : "Outer"}: ${entries.length ? entries.map(row => `${row.id} ${row[key]}`).join(", ") : "no computed counts"}`);
    if (!total) {
      chart.append(svgElement("circle", { cx: 100, cy: 100, r: 92, fill: "#ecf0ed" }));
      const empty = svgElement("text", { x: 100, y: 106, "text-anchor": "middle", fill: "#52615c" });
      empty.textContent = "—";
      chart.append(empty);
      continue;
    }
    let angle = -Math.PI / 2;
    for (const row of entries) {
      const color = speciesColor(row.id);
      const end = angle + 2 * Math.PI * row[key] / total;
      const point = value => `${100 + 92 * Math.cos(value)} ${100 + 92 * Math.sin(value)}`;
      const slice = entries.length === 1
        ? svgElement("circle", { cx: 100, cy: 100, r: 92, fill: color })
        : svgElement("path", { d: `M 100 100 L ${point(angle)} A 92 92 0 ${end - angle > Math.PI ? 1 : 0} 1 ${point(end)} Z`, fill: color, stroke: "white", "stroke-width": 1 });
      slice.dataset.lipid = row.id;
      slice.dataset.count = row[key];
      const title = svgElement("title", {});
      title.textContent = `${row.id}: ${row[key]}`;
      slice.append(title);
      chart.append(slice);
      const item = document.createElement("li");
      const swatch = document.createElement("span");
      swatch.className = "pie-key";
      swatch.style.background = color;
      swatch.setAttribute("aria-hidden", "true");
      const label = document.createElement("span");
      label.textContent = row.id;
      const value = document.createElement("span");
      value.className = "pie-value";
      value.textContent = row[key];
      item.append(swatch, label, value);
      legend.append(item);
      angle = end;
    }
  }
}

function showHover(lipid, anchor) {
  if (busy) return;
  const preview = $("lipid-hover");
  preview.querySelector("strong").textContent = lipid.name;
  const image = preview.querySelector("img");
  image.src = lipid.image;
  image.alt = `${lipid.name} chemical structure`;
  preview.hidden = false;
  const rect = anchor.getBoundingClientRect();
  preview.style.left = `${Math.max(8, Math.min(rect.right + 12, innerWidth - preview.offsetWidth - 8))}px`;
  preview.style.top = `${Math.max(8, Math.min(rect.top, innerHeight - preview.offsetHeight - 8))}px`;
}

function renderRows(focusID) {
  $("lipid-rows").replaceChildren();
  for (const row of rows) {
    const lipid = catalogue.find(item => item.id === row.id);
    const tr = document.createElement("tr");
    const nameCell = document.createElement("th");
    nameCell.scope = "row";
    const name = document.createElement("button");
    name.type = "button";
    name.className = "lipid-name";
    name.textContent = lipid.name;
    name.setAttribute("aria-label", `${lipid.name}: show chemical structure`);
    for (const event of ["mouseenter", "focus", "click"]) name.addEventListener(event, () => showHover(lipid, name));
    for (const event of ["mouseleave", "blur"]) name.addEventListener(event, () => { $("lipid-hover").hidden = true; });
    nameCell.append(name);
    appendTailComposition(nameCell, lipid);
    tr.append(nameCell);
    for (const { key, name: side } of sides) {
      const td = document.createElement("td");
      const input = document.createElement("input");
      input.type = "number";
      input.min = "0";
      input.step = "any";
      input.required = true;
      input.value = row[key];
      input.id = `${row.id}-${side}`;
      input.setAttribute("aria-label", `${row.id} ${side} mole percentage`);
      input.addEventListener("input", () => {
        row[key] = input.valueAsNumber;
        invalidate();
        refreshComposition();
      });
      const normalized = document.createElement("span");
      normalized.id = `${row.id}-${side}-normalized`;
      normalized.className = "normalized-percentage";
      const count = document.createElement("span");
      count.id = `${row.id}-${side}-count`;
      count.className = "lipid-count";
      const adjustment = document.createElement("span");
      adjustment.id = `${row.id}-${side}-adjustment`;
      adjustment.className = "lipid-adjustment";
      adjustment.hidden = true;
      input.setAttribute("aria-describedby", `${normalized.id} ${adjustment.id} ${count.id}`);
      const pair = document.createElement("div");
      pair.className = "percentage-count";
      const field = document.createElement("span");
      field.className = "percentage-field";
      field.append(input);
      pair.append(field, normalized, adjustment, count);
      td.append(pair);
      tr.append(td);
    }
    const removeCell = document.createElement("td");
    const remove = document.createElement("button");
    remove.type = "button";
    remove.className = "secondary remove-lipid";
    remove.textContent = "×";
    remove.setAttribute("aria-label", `Remove ${row.id}`);
    remove.addEventListener("click", () => {
      rows = rows.filter(item => item !== row);
      invalidate();
      renderRows();
      $("add-lipid").focus();
    });
    removeCell.append(remove);
    tr.append(removeCell);
    $("lipid-rows").append(tr);
  }
  refreshComposition();
  if (focusID) $(focusID)?.focus();
}

function appendTailComposition(container, lipid) {
  if (!lipid.tails) return;
  const tails = document.createElement("span");
  tails.className = "small lipid-tail";
  tails.textContent = lipid.tails;
  container.append(tails);
}

function previewLipid(lipid) {
  $("preview-content").hidden = false;
  $("preview-name").textContent = lipid.name;
  $("preview-tails").textContent = lipid.tails;
  $("preview-tails").hidden = !lipid.tails;
  $("preview-image").hidden = false;
  $("preview-error").hidden = true;
  $("preview-image").src = lipid.image;
  $("preview-image").alt = `${lipid.name} chemical structure`;
}
function hidePreview() { $("preview-content").hidden = true; }
$("preview-image").addEventListener("error", () => {
  $("preview-image").hidden = true;
  $("preview-error").hidden = false;
});

function renderChoices(container, choices, selected, field, controls, select) {
  container.replaceChildren();
  for (const choice of choices) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "secondary picker-choice";
    button.dataset[field] = choice;
    button.textContent = choice;
    button.setAttribute("aria-pressed", String(choice === selected));
    button.setAttribute("aria-controls", controls);
    button.addEventListener("click", () => {
      select(choice);
      renderPicker();
      container.querySelector('[aria-pressed="true"]')?.focus();
    });
    container.append(button);
  }
}

function renderPicker(preferExact = false) {
  hidePreview();
  const search = $("lipid-search").value.trim().toLowerCase();
  const terms = search.split(/\s+/);
  const matches = catalogue.filter(lipid => {
    const fields = [lipid.id, lipid.name, lipid.family, lipid.subfamily, lipid.chemical_subfamily, lipid.group, lipid.tails].join(" ").toLowerCase();
    return terms.every(term => fields.includes(term));
  });
  if (preferExact && search) {
    const exact = matches.find(lipid => lipid.id.toLowerCase() === search)
      ?? matches.find(lipid => lipid.subfamily.toLowerCase() === search);
    if (exact) {
      selectedFamily = exact.family;
      selectedSubfamily = exact.subfamily;
    }
  }
  const families = [...new Set(matches.map(lipid => lipid.family))];
  if (!families.includes(selectedFamily)) selectedFamily = families[0] ?? null;
  const members = matches.filter(lipid => lipid.family === selectedFamily);
  const subfamilies = [...new Set(members.map(lipid => lipid.subfamily))];
  if (!subfamilies.includes(selectedSubfamily)) selectedSubfamily = subfamilies[0] ?? null;

  renderChoices($("lipid-families"), families, selectedFamily, "family", "lipid-subfamilies", family => {
    if (selectedFamily !== family) selectedSubfamily = null;
    selectedFamily = family;
  });
  renderChoices($("lipid-subfamilies"), subfamilies, selectedSubfamily, "subfamily", "picker-lipids", subfamily => {
    selectedSubfamily = subfamily;
  });
  $("picker-lipids").replaceChildren();
  for (const lipid of members.filter(item => item.subfamily === selectedSubfamily)) {
    const added = rows.some(row => row.id === lipid.id);
    const button = document.createElement("button");
    button.type = "button";
    button.className = "secondary catalog-lipid";
    button.dataset.lipid = lipid.id;
    button.setAttribute("aria-label", `${added ? "Already added" : "Add"} ${lipid.name}${lipid.tails ? ", " + lipid.tails : ""}`);
    button.setAttribute("aria-disabled", String(added));
    const name = document.createElement("span");
    name.textContent = lipid.name;
    appendTailComposition(name, lipid);
    const action = document.createElement("span");
    action.textContent = added ? "Added" : "+";
    action.setAttribute("aria-hidden", "true");
    button.append(name, action);
    button.addEventListener("mouseenter", () => previewLipid(lipid));
    button.addEventListener("focus", () => {
      if (button.matches(":focus-visible")) previewLipid(lipid);
    });
    for (const event of ["mouseleave", "blur"]) button.addEventListener(event, hidePreview);
    button.addEventListener("click", () => {
      if (added) return;
      rows.push({ id: lipid.id, upper: 0, lower: 0 });
      invalidate();
      $("lipid-picker").close();
      renderRows(`${lipid.id}-inner`);
    });
    $("picker-lipids").append(button);
  }
  $("no-lipids").hidden = matches.length > 0;
}

$("add-lipid").addEventListener("click", () => {
  $("lipid-hover").hidden = true;
  $("lipid-search").value = "";
  renderPicker();
  $("lipid-picker").showModal();
  $("lipid-search").focus();
});
$("close-picker").addEventListener("click", () => $("lipid-picker").close());
$("lipid-picker").addEventListener("close", hidePreview);
$("lipid-search").addEventListener("input", () => renderPicker(true));
for (const id of ["per-leaflet", "per-membrane"]) $(id).addEventListener("change", () => {
  invalidate();
  refreshComposition();
});
for (const id of ["cell-size", "xy-margin"]) $(id).addEventListener("input", () => { invalidate(); refreshComposition(); });
$("balancing-lipid").addEventListener("change", () => { invalidate(); refreshComposition(); });
$("balance").addEventListener("click", () => {
  if (!normalizedRows || busy) return;
  if (!baseCounts.length) {
    invalidate(false); balanceSpecies = $("balancing-lipid").value; refreshComposition(); return;
  }
  const adjustment = balanceLipid(baseCounts, catalogue, $("balancing-lipid").value, proteinAreas);
  if (!adjustment) { showError("No whole-lipid adjustment can reduce the area difference."); return; }
  invalidate(false);
  balanceAdjustment = adjustment; balanceSpecies = adjustment.id;
  refreshComposition();
});

const betaTerms = $("public-beta-terms");
$("submission-fee").replaceChildren(...Array.from(betaTerms.children, node => node.cloneNode(true)));

async function connect() {
  ready = false;
  try {
    const response = await fetch(`${api}/health`, { signal: AbortSignal.timeout(30000) });
    if (!response.ok) throw new Error();
    const health = await response.json();
    testCheckout = health.payment_mode === "test";
    paypalMode = ["paypal-sandbox", "paypal-live"].includes(health.payment_mode) ? health.payment_mode : null;
    if (publicBuilder && paypalMode !== "paypal-live") throw new Error();
    if (paypalMode) {
      const amount = health.amount || health.sandbox_amount;
      const price = `${amount.currency_code} ${amount.value}`;
      $("assemble-system").textContent = `Pay ${price} with PayPal and build`;
      for (const label of document.querySelectorAll("[data-build-price]")) label.textContent = price;
    }
    ready = health.ready === true;
    if (Number.isFinite(health.compute_timeout_seconds)) timeoutSeconds = health.compute_timeout_seconds;
    $("connection").textContent = ready ? "" : "Start the membrane server and reload this page.";
  } catch {
    $("connection").textContent = publicBuilder ? "Online checkout is currently unavailable. Try again later." : "Cannot connect. Start the membrane server and reload this page.";
  }
  $("connection").hidden = ready;
  updateBuildButton(); invalidateAssembly();
}

$("build").addEventListener("click", () => {
  if (!normalizedRows || busy || assemblyBusy || !componentsReady()) return;
  membraneRequest = { lipid_weights: rows.map(row => ({ ...row })),
    normalization: $("per-leaflet").checked ? "leaflet" : "total", size_nm: $("cell-size").valueAsNumber,
    xy_margin_nm: $("xy-margin").valueAsNumber };
  syncMembraneStep(); location.hash = "#solvent";
});

async function paymentPlan(assembly) {
  const membrane = $("add-membrane").checked;
  const sources = ligandState.sources || [];
  const plan = {};
  if (proteinState.request) plan.protein = { ...proteinState.request, membrane };
  for (const [index, { item, cif }] of sources.entries()) {
    plan[`ligand-${index}`] = { sha256: await sha256(cif), sdf: cif instanceof Blob,
      source: ligandHeaders(item, cif)["X-Ligand-Source"] || "" };
  }
  if (plan.protein || sources.length) plan.system = { protein_id: plan.protein ? "$protein" : null,
    ligand_ids: sources.map((_, index) => `$ligand-${index}`) };
  if (membrane) plan.membrane = { ...membraneRequest, ...(plan.system ? {complex_id: "$system"} : {}),
    ...(balanceSpecies ? {balancing_lipid: balanceSpecies} : {}) };
  plan.assembly = { ...assembly, ...(membrane ? {membrane_id: "$membrane"} : {complex_id: "$system"}) };
  return plan;
}

async function prepareBatch({ stage, task, request, assembly }) {
  if (paypalMode) {
    stage("Waiting for PayPal approval");
    const cancel = $("cancel-checkout");
    const controller = new AbortController();
    cancel.textContent = "Cancel checkout";
    cancel.hidden = false;
    cancel.onclick = () => controller.abort();
    try {
      await checkoutBuild(() => paymentPlan(assembly), paypalMode, {
        signal: controller.signal,
        quotedPrice: betaTerms.querySelector("[data-build-price]").textContent,
        onConfirming() { cancel.textContent = "Stop waiting"; stage("Confirming PayPal payment"); },
      });
    } finally {
      cancel.hidden = true;
      cancel.onclick = null;
    }
  }
  if (testCheckout) {
    stage("Authorizing free test build");
    const response = await fetch(`${builderApi}/builder/checkout`, {
      method: "POST", headers: { "Content-Type": "application/json" }, body: "{}",
    });
    if (!response.ok) throw new Error("Could not authorize the free test build. Click Build to retry.");
  }
  complex = null; proteinAreas = { upper: 0, lower: 0 }; balanceAdjustment = null;
  const membrane = $("add-membrane").checked;
  if (proteinState.request) stage("Preparing protein", "Reading protein");
  const protein = await buildDeferredProtein({ membrane, request });
  if (ligandState.sources?.length) stage("Parameterizing ligands");
  const ligandIds = await parameterizeDeferredLigands(task);
  if (protein || ligandIds.length) {
    stage("Combining components", "Preparing solute coordinates");
    const response = await fetch(`${builderApi}/system/prepare`, {
      method: "POST", headers: buildHeaders({ "Content-Type": "application/json" }), signal: AbortSignal.timeout(630000),
      body: JSON.stringify({ protein_id: protein?.build_id || null, ligand_ids: ligandIds }),
    });
    const result = await response.json();
    if (!response.ok) throw new Error(result.error || "System preparation failed.");
    complex = result; proteinAreas = result.protein_areas;
  }
  refreshComposition();
  if (!$("add-membrane").checked) return { complex_id: complex.complex_id };
  if (balanceSpecies && baseCounts.length) {
    balanceAdjustment = balanceLipid(baseCounts, catalogue, balanceSpecies, proteinAreas);
    refreshComposition();
  }
  stage("Building membrane", "Preparing membrane input");
  $("composition-controls").hidden = true; $("built-membrane").hidden = true;
  $("computation").hidden = false; $("compute-progress").hidden = false;
  try {
    const response = await request(`${api}/compute`, {
      method: "POST", headers: { "Content-Type": "application/json", "Accept": "multipart/form-data" },
      signal: AbortSignal.timeout((timeoutSeconds + 30) * 1000),
      body: JSON.stringify({ ...membraneRequest, ...(complex ? { complex_id: complex.complex_id } : {}),
        ...(balanceSpecies ? { balancing_lipid: balanceSpecies } : {}) }),
    });
    if (!response.ok) {
      const detail = await response.json().catch(() => ({}));
      throw new Error(detail.error || "The membrane computation failed.");
    }
    const result = await response.formData(), archive = result.get("archive"), structure = result.get("structure");
    if (!archive?.size || !structure?.size) throw new Error("The builder returned an incomplete result.");
    const membraneId = response.headers.get("X-Membrane-ID");
    if (!membraneId) throw new Error("The server did not retain the membrane. Restart the updated server.");
    builtMembrane = membraneId;
    if (resultURL) URL.revokeObjectURL(resultURL);
    resultURL = URL.createObjectURL(archive); unsaved = true;
    $("save-membrane").href = resultURL; $("save-membrane").hidden = false;
    $("built-membrane").hidden = false;
    showMembrane(await structure.arrayBuffer());
    return { membrane_id: membraneId };
  } catch (error) { $("composition-controls").hidden = false; throw error; }
  finally { $("computation").hidden = true; $("compute-progress").hidden = true; }
}

let viewerListener = null;
let viewerTimer = null;
function clearViewer() {
  clearTimeout(viewerTimer);
  if (viewerListener) window.removeEventListener("message", viewerListener);
  viewerListener = null;
  $("membrane-viewer").replaceChildren();
  delete $("membrane-viewer").dataset.state;
}

function showMembrane(bytes) {
  clearViewer();
  const frame = document.createElement("iframe");
  frame.title = "Built membrane";
  frame.src = "./viewer.html";
  const species = baseCounts.filter(row => row.upper + row.lower
    + (balanceAdjustment?.id === row.id ? balanceAdjustment.delta : 0) > 0)
    .map(row => ({ id: row.id, color: speciesColor(row.id) }));
  viewerListener = event => {
    if (event.origin !== location.origin || event.source !== frame.contentWindow) return;
    if (event.data?.type === "ready") {
      frame.contentWindow.postMessage({ type: "file", mode: "membrane", name: "membrane.cif", bytes, species,
        proteinSelection: complex?.protein_selection || null,
        ptms: proteinState.build?.ptmSelection || "",
        disulfides: proteinState.build?.ssSelection || "",
        ligandSelection: complex?.ligand_selection || "not all",
        includeLigands: ligandState.items.length > 0 }, location.origin, [bytes]);
    } else if (event.data?.type === "rendered") {
      clearTimeout(viewerTimer);
      $("membrane-viewer").dataset.state = "rendered";
      showError("");
    } else if (["invalid", "unavailable"].includes(event.data?.type)) {
      clearTimeout(viewerTimer);
      $("membrane-viewer").dataset.state = "unavailable";
      showError(event.data.message || "The membrane viewer is unavailable. You can still save the ZIP or edit the membrane.");
    }
  };
  window.addEventListener("message", viewerListener);
  viewerTimer = setTimeout(() => showError("The membrane viewer did not finish loading. You can still save the ZIP or edit the membrane."), 60000);
  $("membrane-viewer").append(frame);
}

$("edit-membrane").addEventListener("click", () => {
  invalidate(false);
  refreshComposition();
  $("build").focus();
});

function navigate(focus = true) {
  const requested = location.hash.slice(1);
  const gated = name => (name === "proteins" && !loadState.hasStructure) || (name === "ligands" && !ligandState.items.length);
  const pages = ["load", "proteins", "ligands", "membrane", "solvent", "submit"];
  const page = requested === "assembly" ? "submit" : pages.includes(requested) && !gated(requested) ? requested : "load";
  if (requested && requested !== page) history.replaceState(null, "", "#" + page);
  for (const link of document.querySelectorAll("[data-page]")) {
    if (gated(link.dataset.page)) {
      link.setAttribute("aria-disabled", "true"); link.removeAttribute("href");
    } else {
      link.removeAttribute("aria-disabled"); link.href = "#" + link.dataset.page;
    }
    const active = link.dataset.page === page;
    $("page-" + link.dataset.page).hidden = !active;
    if (active) link.setAttribute("aria-current", "page");
    else link.removeAttribute("aria-current");
  }
  $("lipid-hover").hidden = true;
  if (focus) $(page + "-heading").focus();
}
window.addEventListener("hashchange", () => navigate());
window.addEventListener("keydown", event => { if (event.key === "Escape") $("lipid-hover").hidden = true; });
window.addEventListener("scroll", () => { $("lipid-hover").hidden = true; }, true);
window.addEventListener("beforeunload", event => {
  if (busy || unsaved || proteinState.busy || assemblyBusy) { event.preventDefault(); event.returnValue = ""; }
});
$("assembly-download").addEventListener("click", () => {
  if (!$("assembly-ready").hidden) unsaved = false;
});
$("save-membrane").addEventListener("click", () => {
  unsaved = false;
});

function updatePreparedSystem() {
  if (assemblyBusy) return;
  const configuration = [loadState.done, proteinState.pending, JSON.stringify(proteinState.request), ligandState.sources, ligandState.pending];
  const key = [...configuration, loadState.busy, proteinState.busy, ligandState.busy];
  if (key.length === preparationKey.length && key.every((value, i) => value === preparationKey[i])) return;
  // Busy controls affect readiness, but do not change the prepared solute geometry.
  if (configuration.some((value, i) => value !== preparationKey[i])) {
    complex = null; proteinAreas = { upper: 0, lower: 0 }; balanceAdjustment = null;
  }
  preparationKey = key;
  invalidate(false, true);
  if (hasSolute()) $("cell-size").removeAttribute("max"); else $("cell-size").max = 12;
  if (catalogue.length) refreshComposition();
  updateBuildButton();
}
function updateBuildButton() { $("build").disabled = !normalizedRows || busy || assemblyBusy || !componentsReady() || loadState.busy; }
setupProteins(updatePreparedSystem);
setupLoad(); setupAssembly({ ready: batchReady, prepare: prepareBatch, plan: () => {
  const plan = [];
  const protein = proteinState.request;
  if (protein) {
    const details = [];
    if ($("add-membrane").checked && protein.orient) details.push("orientation");
    if (protein.mutations?.length) details.push(...protein.mutations.map(m => `resindex ${m.residue} → ${m.target}`));
    if (protein.ptms.length) details.push(...protein.ptms.map(p => p.iupac ? "glycan" : p.type.replaceAll("_", " ")));
    if (protein.ss_bonds.length) details.push("disulfide bonds");
    plan.push(["Prepare protein", [...new Set(details)].join(", ")]);
  }
  if (ligandState.sources?.length) plan.push(["Parameterize ligands", Array.from(groupedLigands(), ([name, copies]) => ligandGroupLabel(name, copies.length)).join(", ")]);
  if (protein || ligandState.sources?.length) plan.push(["Combine components"]);
  if ($("add-membrane").checked && membraneRequest) plan.push(["Build membrane", membraneRequest.lipid_weights.map(row => row.id).join(", ")]);
  return plan;
}, viewer: () => ({
  species: !$("add-membrane").checked ? [] : baseCounts.filter(row => row.upper + row.lower > 0).map(row => ({id:row.id.slice(0,5),color:speciesColor(row.id)})),
  proteinSelection: proteinState.build?.protein_selection || null,
  ptms: proteinState.build?.ptmSelection || "", disulfides: proteinState.build?.ssSelection || "",
  ligandSelection: complex?.ligand_selection || "not all",
}) });
$("continue-no-membrane").onclick = () => { if (!$("continue-no-membrane").disabled) location.hash = "#solvent"; };
$("add-membrane").addEventListener("change", syncMembraneStep);
syncMembraneStep();
window.addEventListener("load-state", () => { navigate(false); updatePreparedSystem(); });
window.addEventListener("ligands-changed", updatePreparedSystem);
window.addEventListener("protein-busy", updatePreparedSystem);
window.addEventListener("assembly-busy", event => {
  assemblyBusy = event.detail; $("composition-controls").disabled = assemblyBusy;
  $("add-membrane").disabled = assemblyBusy; $("continue-no-membrane").disabled = assemblyBusy || !componentsReady() || !hasSolute(); $("edit-membrane").disabled = assemblyBusy;
  $("lipid-picker").inert = assemblyBusy;
  updateBuildButton();
});
navigate(false);
connect();
try {
  const response = await fetch("./membrane-assets/catalog.json");
  if (!response.ok) throw new Error("The lipid catalogue could not be loaded. Reload the page to retry.");
  catalogue = (await response.json()).lipids;
  renderRows();
  $("add-lipid").disabled = false;
} catch (error) { showError(error.message); }
