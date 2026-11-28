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
pub(super) fn changed(ui: Ui) {
    ui.set_done(1, false);
    ui.set_done(5, false);
}
fn filtered(ui: Ui, kind: &str, query: &str) -> Vec<(String, String)> {
    let rows = inventory(ui);
    let p = ui.plan.get().protein;
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
fn modification_list(ui: Ui, editor: glycan::Editor, kind: &'static str) -> impl IntoView {
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
                    view! { <div class="protein-item">{components::residue_label(&r)}</div> }
                })
                .collect_view()}
            {values
                .into_iter()
                .enumerate()
                .map(|(i, item)| {
                    let site = item["residue"].as_u64().unwrap();
                    let remove = move || {
                        ui.plan
                            .update(|p| {
                                if kind == "ptms" {
                                    p.protein.ptms.remove(i);
                                } else {
                                    p.protein.mutations.remove(i);
                                }
                            });
                        changed(ui);
                    };
                    if item["type"] == "glycosylation" {
                        return editor.row(site, item, remove).into_any();
                    }
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
                        format!("{name}, {}", item["type"].as_str().unwrap_or("").replace('_', " "))
                    };
                    view! {
                        <div class="protein-item">
                            {label}
                            <button
                                class="remove-lipid"
                                aria-label="Remove"
                                on:click=move |_| remove()
                            >
                                "×"
                            </button>
                        </div>
                    }
                        .into_any()
                })
                .collect_view()}
        }
    }
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
                disabled=move || {
                    ui
                        .plan
                        .with(|p| {
                            p
                                .protein
                                .ptms
                                .iter()
                                .any(|m| {
                                    m["residue"]
                                        .as_u64()
                                        .is_some_and(|n| pair.contains(&(n as usize)))
                                        && matches!(
                                            m["type"].as_str(),
                                            Some(
                                                "palmitoylation" | "farnesylation" | "geranylgeranylation"
                                                | "triacylation",
                                            )
                                        )
                                })
                        })
                }
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

const AMINO_ACIDS: &str =
    "ALA ARG ASN ASP CYS GLN GLU GLY HIS ILE LEU LYS MET PHE PRO SER THR TRP TYR VAL";
fn selected_site(rows: &[(String, String)], selected: Option<u64>) -> Option<u64> {
    selected
        .filter(|id| rows.iter().any(|(s, _)| s == &id.to_string()))
        .or_else(|| rows.first()?.0.parse().ok())
}
#[component]
fn MutationPicker(ui: Ui, modal: Modal, query: RwSignal<String>) -> impl IntoView {
    let target = RwSignal::new("ALA".to_string());
    let selected = RwSignal::new(None::<u64>);
    let sites = move || filtered(ui, "", &query.get());
    let submit = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        let Some(site) = selected_site(&sites(), selected.get_untracked()) else {
            return;
        };
        let next = target.get_untracked();
        let Some(r) = inventory(ui).into_iter().find(|r| r["id"] == site) else {
            return;
        };
        let mut p = ui.plan.get_untracked().protein;
        p.mutations.retain(|m| m["residue"] != site);
        if r["name"] != next {
            p.mutations.push(json!({"residue":site,"target":next}));
        }
        modal.close();
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
    };
    view! {
        <dialog id="mutation-picker" node_ref=modal.0>
            <form id="mutation-form" on:submit=submit>
                <h2>"Mutate residue"</h2>
                <label>
                    "Search residues "
                    <input
                        id="mutation-search"
                        type="search"
                        prop:value=move || query.get()
                        on:input=move |e| query.set(event_target_value(&e))
                    />
                </label>
                <label>
                    "Residue "
                    <select
                        id="mutation-residue"
                        on:change=move |e| selected.set(event_target_value(&e).parse::<u64>().ok())
                    >
                        {move || {
                            sites()
                                .into_iter()
                                .map(|(value, label)| {
                                    view! { <option value=value>{label}</option> }
                                })
                                .collect_view()
                        }}
                    </select>
                </label>
                <label>
                    "Mutate to "
                    <select
                        id="mutation-target"
                        on:change=move |e| target.set(event_target_value(&e))
                    >
                        {AMINO_ACIDS
                            .split_whitespace()
                            .map(|s| view! { <option value=s>{s}</option> })
                            .collect_view()}
                    </select>
                </label>
                <div class="actions">
                    <button type="submit">"Apply"</button>
                    <button type="button" class="secondary" on:click=move |_| modal.close()>
                        "Cancel"
                    </button>
                </div>
            </form>
        </dialog>
    }
}

const PTM_TYPES: &[(&str, &str)] = &[
    ("phosphorylation", "Phosphorylation"),
    ("palmitoylation", "Lipidation: Cys S-palmitoylation"),
    (
        "glycine_myristoylation",
        "Lipidation: N-terminal Gly myristoylation",
    ),
    ("lysine_myristoylation", "Lipidation: Lys myristoylation"),
    ("farnesylation", "Lipidation: Cys farnesylation"),
    ("geranylgeranylation", "Lipidation: Cys geranylgeranylation"),
    ("triacylation", "Lipidation: N-terminal Cys triacylation"),
    ("glycosylation", "Glycosilation"),
];

#[component]
fn PtmPicker(
    ui: Ui,
    modal: Modal,
    query: RwSignal<String>,
    kind: RwSignal<String>,
) -> impl IntoView {
    let selected = RwSignal::new(None::<u64>);
    let charge = RwSignal::new(-2i32);
    let sites = move || filtered(ui, &kind.get(), &query.get());
    let not_phosphate = move || kind.get() != "phosphorylation";
    let submit = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        let Some(site) = selected_site(&sites(), selected.get_untracked()) else {
            return;
        };
        let kind = kind.get_untracked();
        let mut m = json!({"type":kind,"residue":site});
        if kind == "glycosylation" {
            m["iupac"] = json!("GlcNAc");
            m["root_anomer"] = json!("b");
        }
        if kind == "phosphorylation" {
            m["charge"] = json!(charge.get_untracked());
        }
        ui.plan.update(|p| p.protein.ptms.push(m));
        changed(ui);
        modal.close();
    };
    view! {
        <dialog id="ptm-picker" aria-labelledby="ptm-heading" node_ref=modal.0>
            <h2 id="ptm-heading">"Add PTM"</h2>
            <form id="ptm-form" on:submit=submit>
                <label for="ptm-type">"PTM type"</label>
                <select
                    id="ptm-type"
                    required
                    on:change=move |e| {
                        kind.set(event_target_value(&e));
                        selected.set(None);
                    }
                >
                    {PTM_TYPES
                        .iter()
                        .map(|&(value, label)| {
                            view! {
                                <option
                                    value=value
                                    prop:selected=move || kind.get() == value
                                    disabled=move || filtered(ui, value, "").is_empty()
                                >
                                    {label}
                                </option>
                            }
                        })
                        .collect_view()}
                </select>
                <label for="ptm-search">"Search residues"</label>
                <input
                    id="ptm-search"
                    type="search"
                    placeholder="Residue number, chain or name"
                    prop:value=move || query.get()
                    on:input=move |e| query.set(event_target_value(&e))
                />
                <label for="ptm-residue">"Residue"</label>
                <select
                    id="ptm-residue"
                    required
                    on:change=move |e| selected.set(event_target_value(&e).parse::<u64>().ok())
                >
                    {move || {
                        sites()
                            .into_iter()
                            .map(|(value, label)| view! { <option value=value>{label}</option> })
                            .collect_view()
                    }}
                </select>
                <p id="ptm-empty" class="small" hidden=move || !sites().is_empty()>
                    "No eligible residues."
                </p>
                <label id="ptm-charge-label" for="ptm-charge" hidden=not_phosphate>
                    "Charge"
                </label>
                <select
                    id="ptm-charge"
                    hidden=not_phosphate
                    disabled=not_phosphate
                    on:change=move |e| charge.set(event_target_value(&e).parse().unwrap_or(-2))
                >
                    <option value="-2">"−2"</option>
                    <option value="-1">"−1"</option>
                </select>
                <div class="actions">
                    <button id="ptm-add" type="submit" disabled=move || sites().is_empty()>
                        "Add"
                    </button>
                    <button class="secondary" type="button" on:click=move |_| modal.close()>
                        "Cancel"
                    </button>
                </div>
            </form>
        </dialog>
    }
}

#[component]
fn DisulfidePicker(
    ui: Ui,
    modal: Modal,
    first: RwSignal<Option<u64>>,
    second: RwSignal<Option<u64>>,
    error: RwSignal<String>,
) -> impl IntoView {
    let submit = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        let (Some(a), Some(b), Some(part)) = (
            first.get_untracked(),
            second.get_untracked(),
            ui.part.get_untracked(),
        ) else {
            return;
        };
        let pair = [a as usize, b as usize];
        if a == b
            || ui.plan.with_untracked(|p| {
                p.protein
                    .ss_bonds
                    .iter()
                    .flatten()
                    .any(|n| pair.contains(n))
            })
        {
            error.set("Choose two different cysteines that are not already linked.".into());
            return;
        }
        let found = part.protein["ss_bonds"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|bond| bond["residues"] == json!(pair) || bond["residues"] == json!([b, a]));
        if !found {
            let range = &part.protein["disulfide_range_nm"];
            error.set(format!(
                "The SG–SG distance must be {:.2}–{:.2} nm.",
                range[0].as_f64().unwrap_or_default(),
                range[1].as_f64().unwrap_or_default()
            ));
            return;
        }
        ui.plan.update(|p| p.protein.ss_bonds.push(pair));
        modal.close();
        changed(ui);
    };
    view! {
        <dialog id="ss-picker" aria-labelledby="ss-heading" node_ref=modal.0>
            <h2 id="ss-heading">"Add SS bond"</h2>
            <form id="ss-form" on:submit=submit>
                <label for="ss-first">"Cysteine"</label>
                <select
                    id="ss-first"
                    required
                    on:change=move |e| first.set(event_target_value(&e).parse().ok())
                >
                    {move || {
                        cysteines(ui)
                            .into_iter()
                            .map(|(value, label)| {
                                let n = value.parse::<u64>().ok();
                                view! {
                                    <option value=value prop:selected=move || first.get() == n>
                                        {label}
                                    </option>
                                }
                            })
                            .collect_view()
                    }}
                </select>
                <label for="ss-second">"Cysteine"</label>
                <select
                    id="ss-second"
                    required
                    on:change=move |e| second.set(event_target_value(&e).parse().ok())
                >
                    {move || {
                        cysteines(ui)
                            .into_iter()
                            .map(|(value, label)| {
                                let n = value.parse::<u64>().ok();
                                view! {
                                    <option value=value prop:selected=move || second.get() == n>
                                        {label}
                                    </option>
                                }
                            })
                            .collect_view()
                    }}
                </select>
                <p
                    id="ss-error"
                    class="error"
                    role="alert"
                    hidden=move || error.with(String::is_empty)
                >
                    {move || error.get()}
                </p>
                <div class="actions">
                    <button type="submit">"Add"</button>
                    <button class="secondary" type="button" on:click=move |_| modal.close()>
                        "Cancel"
                    </button>
                </div>
            </form>
        </dialog>
    }
}

#[component]
pub(super) fn ProteinPage(ui: Ui) -> impl IntoView {
    let mutation = Modal::new();
    let ptm = Modal::new();
    let ss = Modal::new();
    let editor = glycan::Editor::new(ui);
    let mutation_query = RwSignal::new(String::new());
    let ptm_query = RwSignal::new(String::new());
    let kind = RwSignal::new("phosphorylation".to_owned());
    let first = RwSignal::new(None);
    let second = RwSignal::new(None);
    let error = RwSignal::new(String::new());
    let inactive = move || !ui.has_protein() || ui.busy.get();
    let first_kind = move || {
        PTM_TYPES
            .iter()
            .map(|&(key, _)| key)
            .find(|k| !filtered(ui, k, "").is_empty())
    };
    view! {
        <div class="stage-heading">
            <h2 id="proteins-heading" tabindex="-1">
                "2. Protein"
            </h2>
            <button
                class="secondary help-button"
                type="button"
                aria-label="Help for Protein"
                aria-haspopup="dialog"
                aria-controls="stage-help"
                data-help="Choose any changes you want to make to your protein. If you are adding a membrane, select \"Orient\" to align the protein with it."
            >
                "?"
            </button>
        </div>
        <div class="protein-layout">
            <div class="protein-controls">
                <fieldset id="protein-options" disabled=inactive>
                    <div class="protein-section">
                        <div class="protein-section-heading">
                            <h3>"PTMs"</h3>
                            <button
                                id="add-ptm"
                                class="secondary compact"
                                type="button"
                                disabled=move || first_kind().is_none()
                                on:click=move |_| {
                                    ptm_query.set(String::new());
                                    if filtered(ui, &kind.get_untracked(), "").is_empty()
                                        && let Some(k) = first_kind()
                                    {
                                        kind.set(k.into());
                                    }
                                    ptm.open();
                                }
                            >
                                "Add..."
                            </button>
                        </div>
                        <div id="protein-ptms">{modification_list(ui, editor, "ptms")}</div>
                    </div>
                    <div class="protein-section">
                        <div class="protein-section-heading">
                            <h3>"Mutations"</h3>
                            <button
                                id="add-mutation"
                                class="secondary compact"
                                type="button"
                                on:click=move |_| {
                                    mutation_query.set(String::new());
                                    mutation.open();
                                }
                            >
                                "Add..."
                            </button>
                        </div>
                        <div id="protein-mutations">
                            {modification_list(ui, editor, "mutations")}
                        </div>
                    </div>
                    <div class="protein-section">
                        <div class="protein-section-heading">
                            <h3>"SS bonds"</h3>
                            <button
                                id="add-ss"
                                class="secondary compact"
                                type="button"
                                disabled=move || cysteines(ui).len() < 2
                                on:click=move |_| {
                                    let sites = cysteines(ui);
                                    first.set(sites.first().and_then(|r| r.0.parse().ok()));
                                    second.set(sites.get(1).and_then(|r| r.0.parse().ok()));
                                    error.set(String::new());
                                    ss.open();
                                }
                            >
                                "Add..."
                            </button>
                        </div>
                        <div id="protein-ss">
                            {move || {
                                let rows = inventory(ui);
                                ui.part
                                    .get()
                                    .and_then(|p| p.protein["ss_bonds"].as_array().cloned())
                                    .unwrap_or_default()
                                    .into_iter()
                                    .filter_map(|v| bond_control(ui, &rows, v))
                                    .collect_view()
                            }}
                        </div>
                    </div>
                    <label class="protein-check">
                        <input
                            id="orient-protein"
                            type="checkbox"
                            prop:checked=move || ui.plan.with(|p| p.protein.orient)
                            on:change=move |e| {
                                ui.plan.update(|p| p.protein.orient = event_target_checked(&e));
                                changed(ui);
                            }
                        />
                        " Orient"
                    </label>
                    <div class="actions">
                        <button
                            id="build-protein"
                            type="button"
                            disabled=inactive
                            on:click=move |_| {
                                ui.set_done(1, true);
                                ui.go(if ui.has_ligands() { 2 } else { 3 });
                            }
                        >
                            "Continue"
                        </button>
                        <span
                            id="protein-built"
                            class="tree-check"
                            role="status"
                            hidden=move || !ui.confirmed.get()[5] || !ui.has_protein()
                        >
                            "✓"
                        </span>
                    </div>
                </fieldset>
                <p id="protein-status" role="status" hidden></p>
                <progress id="protein-progress" hidden></progress>
                <p
                    id="protein-error"
                    class="error"
                    role="alert"
                    hidden=move || ui.msg.with(String::is_empty)
                >
                    {move || ui.msg.get()}
                </p>
            </div>
            <div id="protein-viewer" class="structure-viewer" aria-label="Protein structure"></div>
        </div>
        <MutationPicker ui modal=mutation query=mutation_query />
        <PtmPicker ui modal=ptm query=ptm_query kind />
        <DisulfidePicker ui modal=ss first second error />
        <glycan::GlycanPicker editor />
    }
}
