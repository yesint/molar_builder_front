import init, { System, start_embedded } from "./vendor/molar_vis/molar_vis.js";

const $ = id => document.getElementById(id);
const notify = message => parent.postMessage(message, location.origin);
let viewer;
let received = false;
let proteinView = null;
let proteinUpdates = Promise.resolve();
let editableLigand = null;
let lastLigandCif = null;
let ligandEditTimer;
let ligandModeTimer;
let lastLigandMode = null;

async function watchLigandMode() {
  try {
    const enabled = await editableLigand.editing();
    if (enabled !== lastLigandMode) {
      lastLigandMode = enabled; notify({ type: "ligand-editing", enabled });
    }
    ligandModeTimer = setTimeout(watchLigandMode, 500);
  } catch { clearTimeout(ligandModeTimer); }
}

function watchLigandEdit() {
  if (!editableLigand) return;
  clearTimeout(ligandEditTimer);
  ligandEditTimer = setTimeout(async () => {
    try {
      const cif = await editableLigand.export_cif();
      if (cif !== lastLigandCif) {
        lastLigandCif = cif; notify({ type: "ligand-changed", cif });
      }
    } catch { notify({ type: "ligand-changed", cif: null }); }
  }, 250);
}
// The native editor consumes these events on its canvas; observe them first.
window.addEventListener("pointerup", watchLigandEdit, true);
window.addEventListener("keyup", watchLigandEdit, true);

async function showMembranePlanes(planes) {
  if (!planes) return;
  const [min, max] = planes.bounds_nm;
  const low = min.map(value => value - 1);
  const high = max.map(value => value + 1);
  const step = Math.max(0.45, (high[0] - low[0]) / 50, (high[1] - low[1]) / (50 * Math.sqrt(3) / 2));
  const rowStep = step * Math.sqrt(3) / 2;
  const rows = ["data_membrane_planes", "loop_", "_atom_site.group_PDB", "_atom_site.id",
    "_atom_site.type_symbol", "_atom_site.label_atom_id", "_atom_site.label_comp_id",
    "_atom_site.label_asym_id", "_atom_site.label_seq_id", "_atom_site.Cartn_x",
    "_atom_site.Cartn_y", "_atom_site.Cartn_z"];
  let serial = 0;
  for (const [plane, z] of planes.z_nm.entries()) {
    for (let row = 0; low[1] + row * rowStep <= high[1]; row++) {
      const y = low[1] + row * rowStep;
      for (let x = low[0] + (row % 2) * step / 2; x <= high[0]; x += step) {
        // mmCIF coordinates are Å; the orientation result and grid spacing are nm.
        rows.push(`HETATM ${++serial} ${plane ? "O" : "N"} DUM ${plane ? "OUTR O" : "INNR I"} ${serial} ${(x * 10).toFixed(3)} ${(y * 10).toFixed(3)} ${(z * 10).toFixed(3)}`);
      }
    }
  }
  const system = System.from_bytes("membrane_planes.cif", new TextEncoder().encode(rows.join("\n") + "\n"));
  try {
    const molecule = viewer.add_mol(system);
    try {
      for (const rep of await molecule.reps()) {
        try {
          await rep.select_text("resname INNR OUTR");
          // Bondless particles: the ball-and-stick sphere scale is 0.25 of VDW.
          rep.style = "ballstick"; rep.color = "element";
        } finally { rep.free(); }
      }
    } finally { molecule.free(); }
  } finally { system.free(); }
}

function nonemptySelection(system, text, optional = false) {
  if (!text?.trim() || text.trim() === "not all") return false;
  try { const selection = system.select(text); selection.free(); return true; }
  catch (error) { if (optional) return false; throw error; }
}

async function updateProteinView(data) {
  // Selection and appearance changes must preserve the scene and camera.
  // A new scene is created only when loading another structure.
  const { system, style, planes } = proteinView;
  const specifications = [
    ["protein", data.protein, style, "chain"],
    ["ligands", data.ligands, proteinView.ligandStyle, "element"],
    ["ptms", data.ptms && data.protein ? `(${data.protein}) and (${data.ptms})` : "", "ballstick", "element"],
    ["disulfides", data.disulfides && data.protein ? `(${data.protein}) and (${data.disulfides})` : "", "ballstick", "element"],
  ].filter(([key, selection]) => {
    try { return nonemptySelection(system, selection); }
    catch (error) {
      const label = { protein: "Protein", ligands: "Ligand", ptms: "PTM", disulfides: "SS bond" }[key];
      throw new Error(`${label} selection: ${error.message}`);
    }
  });
  if (system && !proteinView.molecule) {
    // Start without a default Lines representation; every role is created fully styled.
    proteinView.molecule = viewer.add_mol(system, true);
    await showMembranePlanes(planes);
  }
  let addedLigand = false;
  for (const { key, selected, file } of data.externalLigands || []) {
    let entry = proteinView.external.get(key);
    if (!entry && selected) {
      const ligand = System.from_bytes("ligand.sdf", new Uint8Array(await file.arrayBuffer()));
      try { entry = { molecule: viewer.add_mol(ligand, true), rep: null }; }
      finally { ligand.free(); }
      proteinView.external.set(key, entry);
      addedLigand = true;
    }
    if (selected && !entry.rep) entry.rep = await entry.molecule.add_rep_text("all", "ballstick", "element");
    if (!selected && entry?.rep) {
      await entry.rep.remove(); entry.rep.free(); entry.rep = null;
    }
  }
  const keys = new Set(specifications.map(([key]) => key));
  for (const [key, entry] of proteinView.reps) {
    if (!keys.has(key)) {
      await entry.rep.remove(); entry.rep.free(); proteinView.reps.delete(key);
    }
  }
  for (const [key, selection, repStyle, color] of specifications) {
    const entry = proteinView.reps.get(key);
    if (entry) {
      if (entry.selection !== selection) {
        await entry.rep.select_text(selection); entry.selection = selection;
      }
    } else {
      const rep = await proteinView.molecule.add_rep_text(selection, repStyle, color);
      proteinView.reps.set(key, { rep, selection });
    }
  }
  if (addedLigand && data.type === "protein-selection") (await viewer.reset_view(0)).free();
}

function unavailable(cause) {
  const error = cause?.reason || cause?.error || cause;
  const message = typeof error === "string" ? error : error?.message;
  $("status").textContent = message || "Preview unavailable.";
  $("status").hidden = false;
  notify({ type: "unavailable", ...(message ? { message } : {}) });
}

// Rust panics are reported as browser errors and may leave a promise pending.
window.addEventListener("error", unavailable);
window.addEventListener("unhandledrejection", unavailable);

window.addEventListener("message", async event => {
  if (event.source !== parent || event.origin !== location.origin) return;
  if (event.data?.type === "ligand-edit" && editableLigand) {
    try {
      await editableLigand.set_editing(Boolean(event.data.enabled));
      notify({ type: "ligand-editing", enabled: Boolean(event.data.enabled) });
    } catch (error) { notify({ type: "invalid", message: error.message }); }
    return;
  }
  if (event.data?.type === "export-ligand") {
    try {
      if (!editableLigand) throw new Error("The ligand viewer is not ready.");
      const cif = await editableLigand.export_cif();
      notify({ type: "ligand-export", requestId: event.data.requestId, cif });
    } catch (error) {
      notify({ type: "ligand-export", requestId: event.data.requestId, error: error.message });
    }
    return;
  }
  if (event.data?.type === "protein-selection" && proteinView) {
    proteinUpdates = proteinUpdates.then(async () => {
      await updateProteinView(event.data);
      notify({ type: "selection-valid", requestId: event.data.requestId });
    }).catch(error => notify({ type: "selection-error", requestId: event.data.requestId, message: error.message }));
    return;
  }
  if (event.data?.type !== "file" || received) return;
  received = true;
  if (event.data.mode === "ligand") {
    try {
      await init();
      viewer = await start_embedded("molecule");
      await viewer.set_ui_visibility(false, true);
      await viewer.set_theme("light");
      viewer.background(1, 1, 1);
      editableLigand = await viewer.add_editable(event.data.name, new Uint8Array(event.data.bytes));
      await editableLigand.set_editing(false);
      for (const rep of await editableLigand.reps()) {
        try {
          await rep.select_text("all");
          rep.style = "ballstick"; rep.color = "element";
          await rep.set_ballstick_bonds(0.15, 0.5);
        } finally { rep.free(); }
      }
      const animation = await viewer.reset_view(0); animation.free();
      lastLigandCif = await editableLigand.export_cif().catch(() => null);
      $("status").hidden = true;
      notify({ type: "rendered", initialLigandCif: lastLigandCif });
      watchLigandMode();
    } catch (error) {
      unavailable(`Cannot display the ligand: ${error.message || error}`);
    }
    return;
  }
  if (event.data.mode === "protein") {
    try {
      await init();
      const system = event.data.bytes ? System.from_bytes(event.data.name, new Uint8Array(event.data.bytes)) : null;
      viewer = await start_embedded("molecule");
      await viewer.set_ui_visibility(false, true);
      await viewer.set_theme("light"); viewer.background(1, 1, 1);
      proteinView = { system, style: event.data.proteinStyle || "cartoon", ligandStyle: event.data.ligandStyle || "licorice",
        planes: event.data.membranePlanes,
        reps: new Map(), external: new Map(), molecule: null };
      proteinUpdates = updateProteinView(event.data);
      await proteinUpdates;
      const animation = await viewer.reset_view(0); animation.free();
      if (event.data.membranePlanes) {
        const rotation = await viewer.rotate(0, 70, 0); rotation.free();
      }
      $("status").hidden = true;
      notify({ type: "rendered" });
    } catch (error) {
      unavailable(`Cannot display the structure: ${error.message || error}`);
    }
    return;
  }
  if (["membrane", "assembly"].includes(event.data.mode)) {
    await displayBuiltSystem(event.data);
  }
});

async function displayBuiltSystem(data) {
  const progress = message => {
    if (data.mode === "assembly") notify({ type: "viewer-progress", message });
    else $("status").textContent = message + "…";
  };
  try {
    progress("Starting MolAR");
    await init();
    progress("Reading coordinates");
    const system = System.from_bytes(data.name, new Uint8Array(data.bytes));
    try {
      progress("Starting graphics");
      viewer = await start_embedded("molecule");
      await Promise.all([viewer.set_ui_visibility(false, true), viewer.set_theme("light")]);
      viewer.background(1, 1, 1);
      progress("Preparing representations");
      const specs = data.species.map(species => [`resname ${species.id}`, "licorice", species.color]);
      if (data.proteinSelection) {
        specs.push([data.proteinSelection, "cartoon", "chain"]);
        for (const detail of [data.ptms, data.disulfides]) {
          if (detail) specs.push([`(${data.proteinSelection}) and (${detail})`, "ballstick", "element"]);
        }
      }
      specs.push([data.ligandSelection, "ballstick", "element"]);
      if (data.mode === "assembly") specs.push(["resname SOL", "lines", "element"], ["resname SOD POT CAL MG CLA", "vdw", "element"]);
      const selected = specs.filter(([selection]) => nonemptySelection(system, selection, true));
      const molecule = viewer.add_mol(system, true);
      await molecule.show_box(true);
      try {
        await Promise.all(selected.map(async ([selection, style, color]) => {
          const rep = await molecule.add_rep_text(selection, style, color);
          try {
            if (style === "ballstick") await rep.set_ballstick_bonds(0.15, 0.5);
          } finally { rep.free(); }
        }));
        progress("Fitting the simulation box");
        const animation = await viewer.reset_view(0);
        animation.free();
        $("status").hidden = true;
        notify({ type: "rendered" });
      } finally { molecule.free(); }
    } finally { system.free(); }
  } catch (error) {
    unavailable(`Cannot display the ${data.mode === "assembly" ? "assembled system" : "membrane"}: ${error.message || error}`);
  }
}

const saved = new URLSearchParams(location.hash.slice(1));
if (parent === window && saved.has("assembly")) {
  try {
    const id = saved.get("assembly");
    if (!/^[0-9a-f]{32}$/.test(id)) throw new Error("Invalid assembly identifier.");
    const { builderApi } = await import("./builder-api.js");
    const response = await fetch(`${builderApi}/assembly/${id}/system.cif`);
    if (!response.ok) throw new Error(`Loading assembled coordinates failed (HTTP ${response.status}).`);
    await displayBuiltSystem({mode: "assembly", name: "system.cif", bytes: await response.arrayBuffer(),
      species: JSON.parse(saved.get("species") || "[]"), proteinSelection: saved.get("proteinSelection") || "protein", ptms: saved.get("ptms"),
      ligandSelection: saved.get("ligandSelection"), disulfides: saved.get("disulfides")});
  } catch (error) { unavailable(error); }
} else {
  notify({ type: "ready" });
}
