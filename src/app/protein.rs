use super::*;
use crate::viewer::{Layer, Preview};
fn can_modify(protein: &Protein, residues: &[Value], id: u64, kind: &str) -> bool {
    let Some(residue) = residues.iter().find(|r| r["id"] == id) else {
        return false;
    };
    if residue["coarse_grained"] == true || protein.ptms.iter().any(|m| m["residue"] == id) {
        return false;
    }
    let name = protein
        .mutations
        .iter()
        .find(|m| m["residue"] == id)
        .and_then(|m| m["target"].as_str())
        .or(residue["name"].as_str())
        .unwrap_or("");
    let free_cysteine = name == "CYS"
        && !protein
            .ss_bonds
            .iter()
            .flatten()
            .any(|&bond| bond as u64 == id);
    match kind {
        "phosphorylation" => matches!(name, "SER" | "THR" | "TYR"),
        "palmitoylation" | "farnesylation" | "geranylgeranylation" => free_cysteine,
        "triacylation" => free_cysteine && residue["n_terminal"] == true,
        "lysine_myristoylation" => name == "LYS",
        "glycine_myristoylation" => name == "GLY" && residue["n_terminal"] == true,
        "glycosylation" => matches!(name, "ASN" | "SER" | "THR"),
        _ => false,
    }
}
fn inventory(ui: Ui) -> Vec<Value> {
    ui.part.with(|p| {
        p.as_ref()
            .and_then(|p| p.protein["residues"].as_array())
            .cloned()
            .unwrap_or_default()
    })
}
pub(super) fn details(ui: Ui, built: bool) -> Vec<Layer> {
    let Some(part) = ui.part.get() else {
        return vec![];
    };
    let rows = inventory(ui);
    let settings = ui.plan.get().protein;
    let mut ptms: Vec<_> = part.protein["ptms"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| r["id"].as_u64())
        .collect();
    ptms.extend(settings.ptms.iter().filter_map(|r| r["residue"].as_u64()));
    let bonds = settings
        .ss_bonds
        .into_iter()
        .flatten()
        .map(|i| i as u64)
        .collect();
    [("ptms", ptms), ("disulfides", bonds)]
        .into_iter()
        .filter_map(|(role, ids)| {
            let values = rows
                .iter()
                .filter(|r| r["protein"] == true)
                .enumerate()
                .filter(|(_, r)| r["id"].as_u64().is_some_and(|id| ids.contains(&id)))
                .filter_map(|(i, r)| {
                    if built {
                        Some(i as u64)
                    } else {
                        r["preview_index"].as_u64().or(r["id"].as_u64())
                    }
                })
                .map(|v| v.to_string())
                .collect::<Vec<_>>();
            (!values.is_empty()).then(|| {
                Layer::new(
                    role,
                    format!("resindex {}", values.join(" ")),
                    "ballstick",
                    "element",
                )
            })
        })
        .collect()
}
pub(super) fn preview(ui: Ui) {
    let Some(p) = ui.part.get() else {
        ui.viewer.send(Action::Preview("protein-viewer", vec![]));
        return;
    };
    if p.protein.is_null() {
        ui.viewer.send(Action::Preview("protein-viewer", vec![]));
        return;
    }
    let completed = ui.result.get().filter(|_| ui.confirmed.get()[5]);
    let prepared = completed.as_ref().and_then(|r| r.prepared.clone());
    let report = completed
        .as_ref()
        .filter(|_| prepared.is_some())
        .map(|r| &r.geometry["prepared"])
        .unwrap_or(&p.protein);
    let main = report["protein_selection"].as_str().unwrap_or("all");
    let mut layers = vec![Layer::new(
        "protein",
        main.into(),
        if prepared.is_none() && p.protein["coarse_grained"] == true {
            "vdw"
        } else {
            "cartoon"
        },
        "chain",
    )];
    layers.extend(details(ui, prepared.is_some()).into_iter().map(|mut l| {
        l.selection = format!("({main}) and ({})", l.selection);
        l
    }));
    let key = completed.as_ref().map(|r| r.key.as_str()).unwrap_or("");
    let path = if prepared.is_some() {
        format!("{key}/prepared.cif")
    } else {
        format!("/api/file/part/{}/protein.cif", p.id)
    };
    let mut models = vec![Preview {
        path,
        name: "protein.cif".into(),
        layers,
        bytes: prepared,
    }];
    if let Some(bytes) = plane_coordinates(&report["membrane_planes"]) {
        models.push(Preview {
            path: format!("{key}/membrane_planes.cif"),
            name: "membrane_planes.cif".into(),
            bytes: Some(bytes.into()),
            layers: vec![Layer::new(
                "planes",
                "resname INNR OUTR".into(),
                "ballstick",
                "element",
            )],
        });
    }
    ui.viewer.send(Action::Preview("protein-viewer", models));
}
fn changed(ui: Ui) {
    ui.set_done(1, false);
    ui.set_done(5, false);
}
fn filtered(ui: Ui, kind: &str, query: &str) -> Vec<(String, String)> {
    let rows = inventory(ui);
    let p = ui.plan.get_untracked().protein;
    let q = query.to_lowercase();
    rows.iter()
        .filter(|r| r["protein"] == true && r["coarse_grained"] != true)
        .filter(|r| kind.is_empty() || can_modify(&p, &rows, r["id"].as_u64().unwrap(), kind))
        .filter_map(|r| {
            let mut display = r.clone();
            if !kind.is_empty()
                && let Some(mutation) = p.mutations.iter().find(|m| m["residue"] == r["id"])
            {
                display["name"] = mutation["target"].clone();
            }
            let label = components::residue_label(&display);
            let matches = q.split_whitespace().all(|term| {
                if !kind.is_empty() && term.len() == 1 && term.as_bytes()[0].is_ascii_alphabetic() {
                    r["chain"]
                        .as_str()
                        .is_some_and(|c| c.trim().eq_ignore_ascii_case(term))
                } else {
                    label.to_lowercase().contains(term)
                }
            });
            matches.then(|| (r["id"].to_string(), label))
        })
        .collect()
}
fn refresh_ptm(ui: Ui) {
    let k = select("ptm-type").value();
    let rows = filtered(ui, &k, &field("ptm-search").value());
    enabled("ptm-add", !rows.is_empty());
    visible("ptm-empty", rows.is_empty());
    options("ptm-residue", rows);
    for id in ["ptm-charge", "ptm-charge-label"] {
        visible(id, k == "phosphorylation");
    }
}
fn id(select_id: &str) -> Option<u64> {
    select(select_id).value().parse().ok()
}
pub(super) fn setup(ui: Ui) {
    let editor = glycan::setup(ui);
    let names = "ALA ARG ASN ASP CYS GLN GLU GLY HIS ILE LEU LYS MET PHE PRO SER THR TRP TYR VAL";
    options(
        "mutation-target",
        names.split_whitespace().map(|n| (n.into(), n.into())),
    );
    click("add-mutation", move || {
        field("mutation-search").set_value("");
        options("mutation-residue", filtered(ui, "", ""));
        let _ = dialog("mutation-picker").show_modal();
    });
    on("mutation-search", "input", move |_| {
        options(
            "mutation-residue",
            filtered(ui, "", &field("mutation-search").value()),
        )
    });
    on("mutation-form", "submit", move |e| {
        e.prevent_default();
        let Some(site) = id("mutation-residue") else {
            return;
        };
        let next = select("mutation-target").value();
        let Some(r) = inventory(ui).into_iter().find(|r| r["id"] == site) else {
            return;
        };
        let mut p = ui.plan.get_untracked().protein;
        p.mutations.retain(|m| m["residue"] != site);
        if r["name"] != next {
            p.mutations.push(json!({"residue":site,"target":next}));
        }
        dialog("mutation-picker").close();
        if next != "CYS" && p.ss_bonds.iter().flatten().any(|&id| id as u64 == site) {
            ui.msg
                .set("Disable the disulfide bond before changing this cysteine.".into());
            return;
        }
        if let Some(modification) = p.ptms.iter().find(|m| m["residue"] == site).cloned() {
            let mut test = p.clone();
            test.ptms.retain(|m| m["residue"] != site);
            if !can_modify(
                &test,
                &inventory(ui),
                site,
                modification["type"].as_str().unwrap_or(""),
            ) {
                ui.msg
                    .set("Remove the incompatible PTM before changing this residue.".into());
                return;
            }
        }
        ui.plan.update(|plan| plan.protein = p);
        changed(ui);
    });
    click("add-ptm", move || {
        field("ptm-search").set_value("");
        refresh_ptm(ui);
        let _ = dialog("ptm-picker").show_modal();
    });
    on("ptm-type", "change", move |_| refresh_ptm(ui));
    on("ptm-search", "input", move |_| refresh_ptm(ui));
    on("ptm-form", "submit", move |e| {
        e.prevent_default();
        let Some(site) = id("ptm-residue") else {
            return;
        };
        let kind = select("ptm-type").value();
        dialog("ptm-picker").close();

        let mut m = json!({"type":kind,"residue":site});
        if kind == "glycosylation" {
            m["iupac"] = json!("GlcNAc");
            m["root_anomer"] = json!("b");
        }
        if kind == "phosphorylation" {
            m["charge"] = json!(select("ptm-charge").value().parse::<i32>().unwrap_or(-2));
        }
        ui.plan.update(|p| p.protein.ptms.push(m));
        changed(ui);
    });
    click("add-ss", move || {
        let sites = cysteines(ui);
        options("ss-first", sites.clone());
        options("ss-second", sites);
        select("ss-second").set_selected_index(1);
        visible("ss-error", false);
        let _ = dialog("ss-picker").show_modal();
    });
    on("ss-form", "submit", move |e| {
        e.prevent_default();
        let (Some(a), Some(b)) = (id("ss-first"), id("ss-second")) else {
            return;
        };
        let pair = [a as usize, b as usize];
        let Some(part) = ui.part.get_untracked() else {
            return;
        };
        if a == b
            || ui
                .plan
                .get_untracked()
                .protein
                .ss_bonds
                .iter()
                .flatten()
                .any(|v| pair.contains(v))
        {
            notice(
                "ss-error",
                "Choose two different cysteines that are not already linked.",
            );
            return;
        }
        let found = part.protein["ss_bonds"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|bond| bond["residues"] == json!(pair) || bond["residues"] == json!([b, a]));
        if !found {
            let range = &part.protein["disulfide_range_nm"];
            notice(
                "ss-error",
                &format!(
                    "The SG–SG distance must be {:.2}–{:.2} nm.",
                    range[0].as_f64().unwrap_or_default(),
                    range[1].as_f64().unwrap_or_default()
                ),
            );
            return;
        }
        ui.plan.update(|p| p.protein.ss_bonds.push(pair));
        dialog("ss-picker").close();
        changed(ui);
    });
    for (kind, host) in [("ptms", "protein-ptms"), ("mutations", "protein-mutations")] {
        mount(host, move || {
            move || {
                let p = ui.plan.get().protein;
                let rows = inventory(ui);
                let values = if kind == "ptms" { p.ptms } else { p.mutations };
                let existing = ui
                    .part
                    .get()
                    .and_then(|p| p.protein["ptms"].as_array().cloned())
                    .unwrap_or_default();
                view! {
                    {existing
                        .into_iter()
                        .filter(|_| kind == "ptms")
                        .map(|r| {
                            view! {
                                <div class="protein-item">{components::residue_label(&r)}</div>
                            }
                        })
                        .collect_view()}
                    {values
                        .into_iter()
                        .enumerate()
                        .map(|(i, item)| {
                            let site = item["residue"].as_u64().unwrap();
                            let name = rows
                                .iter()
                                .find(|r| r["id"] == site)
                                .map(components::residue_label)
                                .unwrap_or_default();
                            let label = if kind == "mutations" {
                                format!("{name} → {}", item["target"].as_str().unwrap_or(""))
                            } else if item["type"] == "phosphorylation" {
                                format!("{name}, phosphate {}", item["charge"])
                            } else {
                                format!(
                                    "{name}, {}",
                                    item["iupac"]
                                        .as_str()
                                        .or(item["type"].as_str())
                                        .unwrap_or("")
                                        .replace('_', " "),
                                )
                            };
                            view! {
                                <div class="protein-item">
                                    {label}
                                    {(item["type"] == "glycosylation")
                                        .then(|| {
                                            view! {
                                                <button
                                                    class="secondary compact"
                                                    on:click=move |_| editor.open(site)
                                                >
                                                    "Edit…"
                                                </button>
                                            }
                                        })}
                                    <button
                                        class="remove-lipid"
                                        aria-label="Remove"
                                        on:click=move |_| {
                                            ui.plan
                                                .update(|p| {
                                                    if kind == "ptms" {
                                                        p.protein.ptms.remove(i);
                                                    } else {
                                                        p.protein.mutations.remove(i);
                                                    }
                                                });
                                            changed(ui);
                                        }
                                    >
                                        "×"
                                    </button>
                                </div>
                            }
                        })
                        .collect_view()}
                }
            }
        });
    }
    mount("protein-ss", move || {
        move || {
            let rows = inventory(ui);
            let candidates = ui
                .part
                .get()
                .and_then(|p| p.protein["ss_bonds"].as_array().cloned())
                .unwrap_or_default();
            candidates
                .into_iter()
                .filter_map(|candidate| bond_control(ui, &rows, candidate))
                .collect_view()
        }
    });
    on("orient-protein", "change", move |_| {
        ui.plan
            .update(|p| p.protein.orient = field("orient-protein").checked());
        changed(ui);
    });
    click("build-protein", move || {
        ui.set_done(1, true);
        ui.go(if ui.has_ligands() { 2 } else { 3 });
    });
    Effect::new(move |_| {
        let active = ui.has_protein() && !ui.busy.get();
        enabled("protein-options", active);
        enabled("build-protein", active);
        enabled("add-ss", cysteines(ui).len() >= 2);
        enabled(
            "add-ptm",
            [
                "phosphorylation",
                "palmitoylation",
                "glycine_myristoylation",
                "lysine_myristoylation",
                "glycosylation",
            ]
            .iter()
            .any(|k| !filtered(ui, k, "").is_empty()),
        );
        field("orient-protein").set_checked(ui.plan.get().protein.orient);
    });
}
fn bond_control(ui: Ui, rows: &[Value], b: Value) -> Option<impl IntoView + use<>> {
    let pair: [usize; 2] = serde_json::from_value(b["residues"].clone()).ok()?;
    let names = pair.map(|id| {
        rows.iter()
            .find(|r| r["id"] == id)
            .map(components::residue_label)
            .unwrap_or_default()
    });
    Some(view! {
        <label class="protein-check">
            <input
                type="checkbox"
                prop:checked=move || ui.plan.with(|p| p.protein.ss_bonds.contains(&pair))
                on:change=move |e| {
                    ui.plan
                        .update(|p| {
                            p.protein.ss_bonds.retain(|b| b != &pair);
                            if event_target_checked(&e) {
                                p.protein.ss_bonds.push(pair);
                            }
                        });
                    changed(ui);
                }
            />
            {format!(
                "{} – {} {:.3} nm",
                names[0],
                names[1],
                b["distance_nm"].as_f64().unwrap_or_default(),
            )}
        </label>
    })
}
fn cysteines(ui: Ui) -> Vec<(String, String)> {
    let p = ui.plan.get().protein;
    inventory(ui)
        .into_iter()
        .filter(|r| {
            let target = p
                .mutations
                .iter()
                .find(|m| m["residue"] == r["id"])
                .and_then(|m| m["target"].as_str())
                .or(r["name"].as_str());
            target == Some("CYS") && !p.ptms.iter().any(|m| m["residue"] == r["id"])
        })
        .map(|r| (r["id"].to_string(), components::residue_label(&r)))
        .collect()
}

fn plane_coordinates(planes: &Value) -> Option<Vec<u8>> {
    use std::fmt::Write;
    let [low, high]: [[f64; 3]; 2] = serde_json::from_value(planes["bounds_nm"].clone()).ok()?;
    let zs: [f64; 2] = serde_json::from_value(planes["z_nm"].clone()).ok()?;
    let low = low.map(|v| v - 1.);
    let high = high.map(|v| v + 1.);
    let hex = 3f64.sqrt() / 2.;
    let step = 0.45f64
        .max((high[0] - low[0]) / 50.)
        .max((high[1] - low[1]) / (50. * hex));
    let mut cif = String::from(
        "data_membrane_planes\nloop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\n",
    );
    let mut serial = 0;
    for (i, z) in zs.into_iter().enumerate() {
        let (element, residue, chain) = [("N", "INNR", "I"), ("O", "OUTR", "O")][i];
        let mut row = 0;
        while low[1] + f64::from(row) * step * hex <= high[1] {
            let y = low[1] + f64::from(row) * step * hex;
            let mut x = low[0] + f64::from(row % 2) * step / 2.;
            while x <= high[0] {
                serial += 1;
                writeln!(
                    cif,
                    "HETATM {serial} {element} DUM {residue} {chain} {serial} {:.3} {:.3} {:.3}",
                    x * 10.,
                    y * 10.,
                    z * 10.
                )
                .ok()?;
                x += step;
            }
            row += 1;
        }
    }
    Some(cif.into_bytes())
}
