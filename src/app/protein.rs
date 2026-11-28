use super::*;
use crate::viewer::{Layer, Preview};
fn can_modify(protein: &Protein, residue: &Residue, kind: &Modification) -> bool {
    let id = residue.id;
    if residue.coarse_grained || protein.ptms.iter().any(|m| m.residue == id) {
        return false;
    }
    let name = protein
        .mutations
        .iter()
        .find(|m| m.residue == id)
        .map(|m| m.target.as_str())
        .unwrap_or(&residue.name);
    let free_cysteine = name == "CYS"
        && !protein
            .ss_bonds
            .iter()
            .flatten()
            .any(|&bond| bond as u64 == id);
    match kind {
        Modification::Phosphorylation { .. } => matches!(name, "SER" | "THR" | "TYR"),
        Modification::Palmitoylation
        | Modification::Farnesylation
        | Modification::Geranylgeranylation => free_cysteine,
        Modification::Triacylation => free_cysteine && residue.n_terminal,
        Modification::LysineMyristoylation => name == "LYS",
        Modification::GlycineMyristoylation => name == "GLY" && residue.n_terminal,
        Modification::Glycosylation(_) => matches!(name, "ASN" | "SER" | "THR"),
    }
}
fn inventory(ui: Ui) -> Vec<Residue> {
    ui.part.with(|p| {
        p.as_ref()
            .and_then(|p| p.protein.as_ref())
            .map(|p| p.residues.clone())
            .unwrap_or_default()
    })
}
pub(super) fn details(ui: Ui, built: bool) -> Vec<Layer> {
    let Some(part) = ui.part.get() else {
        return vec![];
    };
    let rows = inventory(ui);
    let settings = ui.plan.get().protein;
    let mut ptms: Vec<_> = part
        .protein
        .iter()
        .flat_map(|p| p.ptms.iter().map(|r| r.id))
        .collect();
    ptms.extend(settings.ptms.iter().map(|r| r.residue));
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
                .filter(|r| r.protein)
                .enumerate()
                .filter(|(_, r)| ids.contains(&r.id))
                .map(|(i, r)| {
                    if built {
                        i as u64
                    } else {
                        r.preview_index.unwrap_or(r.id)
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
        ui.viewer.send(Action::Preview(Panel::Protein, vec![]));
        return;
    };
    let Some(inventory) = &p.protein else {
        ui.viewer.send(Action::Preview(Panel::Protein, vec![]));
        return;
    };
    let completed = ui
        .result
        .get()
        .filter(|_| ui.confirmed.get()[Page::Submit as usize]);
    let prepared = completed.as_ref().and_then(|r| r.prepared.clone());
    let report = completed
        .as_ref()
        .filter(|_| prepared.is_some())
        .map(|r| &r.geometry["prepared"])
        .unwrap_or(&Value::Null);
    let main = report["protein_selection"]
        .as_str()
        .or(inventory.protein_selection.as_deref())
        .unwrap_or("all");
    let mut layers = vec![Layer::new(
        "protein",
        main.into(),
        if prepared.is_none() && inventory.coarse_grained {
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
    ui.viewer.send(Action::Preview(Panel::Protein, models));
}
pub(super) fn changed(ui: Ui) {
    ui.error(Page::Protein).set(String::new());
    ui.set_done(Page::Protein, false);
    ui.invalidate_build();
}
fn filtered(ui: Ui, kind: Option<&Modification>, query: &str) -> Vec<(String, String)> {
    let rows = inventory(ui);
    let p = ui.plan.get().protein;
    let q = query.to_lowercase();
    rows.iter()
        .filter(|r| r.protein && !r.coarse_grained)
        .filter(|r| kind.is_none_or(|k| can_modify(&p, r, k)))
        .filter_map(|r| {
            let mut display = r.clone();
            if kind.is_some()
                && let Some(mutation) = p.mutations.iter().find(|m| m.residue == r.id)
            {
                display.name.clone_from(&mutation.target);
            }
            let label = display.label();
            let matches = q.split_whitespace().all(|term| {
                if kind.is_some() && term.len() == 1 && term.as_bytes()[0].is_ascii_alphabetic() {
                    r.chain.trim().eq_ignore_ascii_case(term)
                } else {
                    label.to_lowercase().contains(term)
                }
            });
            matches.then(|| (r.id.to_string(), label))
        })
        .collect()
}
fn residue_name(rows: &[Residue], site: u64) -> String {
    rows.iter()
        .find(|r| r.id == site)
        .map(Residue::label)
        .unwrap_or_default()
}
fn removable_row(label: String, remove: impl Fn() + 'static) -> impl IntoView {
    view! {
        <div class="protein-item">
            {label}<button class="remove-lipid" aria-label="Remove" on:click=move |_| remove()>
                "×"
            </button>
        </div>
    }
}
fn set_mutation(ui: Ui, site: u64, target: Option<String>) {
    let residues = inventory(ui);
    let Some(residue) = residues.iter().find(|r| r.id == site) else {
        return;
    };
    let original = residue.name.as_str();
    let next = target.as_deref().unwrap_or(original);
    let mut protein = ui.plan.get_untracked().protein;
    protein.mutations.retain(|m| m.residue != site);
    if original != next {
        protein.mutations.push(Mutation {
            residue: site,
            target: next.to_owned(),
        });
    }
    if let Some(ptm) = protein.ptms.iter().position(|m| m.residue == site) {
        let existing = protein.ptms.remove(ptm);
        if !can_modify(&protein, residue, &existing.modification) {
            ui.error(Page::Protein)
                .set("Remove the incompatible PTM before changing this residue.".into());
            return;
        }
        protein.ptms.insert(ptm, existing);
    }
    if next != "CYS"
        && protein
            .ss_bonds
            .iter()
            .flatten()
            .any(|&id| id as u64 == site)
    {
        ui.error(Page::Protein)
            .set("Disable the disulfide bond before changing this cysteine.".into());
        return;
    }
    ui.plan.update(|p| p.protein = protein);
    changed(ui);
}
fn mutation_list(ui: Ui) -> impl IntoView {
    move || {
        let rows = inventory(ui);
        ui.plan.with(|plan| {
            plan.protein
                .mutations
                .iter()
                .map(|m| {
                    let site = m.residue;
                    removable_row(
                        format!("{} → {}", residue_name(&rows, site), m.target),
                        move || set_mutation(ui, site, None),
                    )
                })
                .collect_view()
        })
    }
}
fn ptm_list(ui: Ui, editor: glycan::Editor) -> impl IntoView {
    move || {
        let rows = inventory(ui);
        let existing = ui.part.with(|p| {
            p.as_ref()
                .and_then(|p| p.protein.as_ref().map(|p| &p.ptms))
                .into_iter()
                .flatten()
                .map(|r| view! { <div class="protein-item">{r.label()}</div> })
                .collect_view()
        });
        let added = ui.plan.with(|plan| {
            plan.protein
                .ptms
                .iter()
                .map(|ptm| {
                    let site = ptm.residue;
                    let remove = move || {
                        ui.plan
                            .update(|p| p.protein.ptms.retain(|m| m.residue != site));
                        changed(ui);
                    };
                    let description = match &ptm.modification {
                        Modification::Glycosylation(glycan) => {
                            return editor.row(site, glycan, remove).into_any();
                        }
                        Modification::Phosphorylation { charge } => format!("phosphate {charge}"),
                        other => other.label().to_owned(),
                    };
                    removable_row(
                        {
                            let name = rows
                                .iter()
                                .find(|r| r.id == site)
                                .map(|r| {
                                    let mut r = r.clone();
                                    if let Some(m) =
                                        plan.protein.mutations.iter().find(|m| m.residue == site)
                                    {
                                        r.name.clone_from(&m.target);
                                    }
                                    r.label()
                                })
                                .unwrap_or_default();
                            format!("{name}, {description}")
                        },
                        remove,
                    )
                    .into_any()
                })
                .collect_view()
        });
        (existing, added)
    }
}
fn bond_control(ui: Ui, rows: &[Residue], b: Disulfide) -> impl IntoView + use<> {
    let pair = b.residues;
    let names = pair.map(|id| {
        rows.iter()
            .find(|r| r.id == id as u64)
            .map(Residue::label)
            .unwrap_or_default()
    });
    let lipidated = move || {
        ui.plan.with(|p| {
            p.protein.ptms.iter().any(|m| {
                pair.contains(&(m.residue as usize)) && m.modification.uses_cysteine_sulfur()
            })
        })
    };
    view! {
        <label class="protein-check">
            <input
                type="checkbox"
                prop:checked=move || ui.plan.with(|p| p.protein.ss_bonds.contains(&pair))
                disabled=lipidated
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
            {format!("{} – {} {:.3} nm", names[0], names[1], b.distance_nm.unwrap_or_default())}
        </label>
    }
}
fn cysteines(ui: Ui) -> Vec<(String, String)> {
    let p = ui.plan.get().protein;
    inventory(ui)
        .into_iter()
        .filter(|r| {
            let target = p
                .mutations
                .iter()
                .find(|m| m.residue == r.id)
                .map(|m| m.target.as_str())
                .unwrap_or(&r.name);
            target == "CYS" && !p.ptms.iter().any(|m| m.residue == r.id)
        })
        .map(|r| (r.id.to_string(), r.label()))
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
    let sites = move || filtered(ui, None, &query.get());
    let submit = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        let Some(site) = selected_site(&sites(), selected.get_untracked()) else {
            return;
        };
        modal.close();
        set_mutation(ui, site, Some(target.get_untracked()));
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
                        on:input=move |e| {
                            query.set(event_target_value(&e));
                            selected.set(None);
                        }
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
                                .enumerate()
                                .map(|(index, (value, label))| {
                                    {
                                        let id = value.parse::<u64>().ok();
                                        view! {
                                            <option
                                                value=value
                                                prop:selected=move || {
                                                    selected.get().map_or(index == 0, |n| Some(n) == id)
                                                }
                                            >
                                                {label}
                                            </option>
                                        }
                                    }
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

#[component]
fn PtmPicker(
    ui: Ui,
    modal: Modal,
    query: RwSignal<String>,
    kind: RwSignal<Modification>,
) -> impl IntoView {
    let selected = RwSignal::new(None::<u64>);
    let charge = RwSignal::new(-2i32);
    let sites = move || filtered(ui, Some(&kind.get()), &query.get());
    let not_phosphate = move || !kind.with(|m| matches!(m, Modification::Phosphorylation { .. }));
    let submit = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        let Some(site) = selected_site(&sites(), selected.get_untracked()) else {
            return;
        };
        let mut modification = kind.get_untracked();
        if let Modification::Phosphorylation { charge: value } = &mut modification {
            *value = charge.get_untracked();
        }
        ui.plan.update(|p| {
            p.protein.ptms.push(Ptm {
                residue: site,
                modification,
            })
        });
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
                        let key = event_target_value(&e);
                        if let Some((m, _)) = Modification::choices()
                            .into_iter()
                            .find(|(m, _)| m.key() == key)
                        {
                            kind.set(m);
                        }
                        selected.set(None);
                    }
                >
                    {Modification::choices()
                        .into_iter()
                        .map(|(modification, label)| {
                            let value = modification.key();
                            view! {
                                <option
                                    value=value
                                    prop:selected=move || kind.with(|m| m.key() == value)
                                    disabled=move || {
                                        filtered(ui, Some(&modification), "").is_empty()
                                    }
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
                    on:input=move |e| {
                        query.set(event_target_value(&e));
                        selected.set(None);
                    }
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
                            .enumerate()
                            .map(|(index, (value, label))| {
                                let id = value.parse::<u64>().ok();
                                view! {
                                    <option
                                        value=value
                                        prop:selected=move || {
                                            selected.get().map_or(index == 0, |n| Some(n) == id)
                                        }
                                    >
                                        {label}
                                    </option>
                                }
                            })
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
        let Some(inventory) = part.protein else {
            return;
        };
        let candidate = inventory
            .ss_bonds
            .iter()
            .map(|b| b.residues)
            .find(|&ids| ids == pair || ids == [b as usize, a as usize]);
        let Some(pair) = candidate else {
            let [min, max] = inventory.disulfide_range_nm;
            error.set(format!("The SG–SG distance must be {min:.2}–{max:.2} nm."));
            return;
        };
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
    let kind = RwSignal::new(Modification::Phosphorylation { charge: -2 });
    let first = RwSignal::new(None);
    let second = RwSignal::new(None);
    let error = RwSignal::new(String::new());
    let inactive = move || !ui.has_protein() || ui.busy.get();
    let first_kind = move || {
        Modification::choices()
            .into_iter()
            .map(|(m, _)| m)
            .find(|m| !filtered(ui, Some(m), "").is_empty())
    };
    view! {
        <StageHeading ui page=Page::Protein />
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
                                    if filtered(ui, Some(&kind.get_untracked()), "").is_empty()
                                        && let Some(k) = first_kind()
                                    {
                                        kind.set(k);
                                    }
                                    ptm.open();
                                }
                            >
                                "Add..."
                            </button>
                        </div>
                        <div id="protein-ptms">{ptm_list(ui, editor)}</div>
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
                        <div id="protein-mutations">{mutation_list(ui)}</div>
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
                                    .and_then(|p| p.protein.map(|p| p.ss_bonds))
                                    .unwrap_or_default()
                                    .into_iter()
                                    .map(|v| bond_control(ui, &rows, v))
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
                                ui.set_done(Page::Protein, true);
                                ui.go(
                                    if ui.has_ligands() { Page::Ligands } else { Page::Membrane },
                                );
                            }
                        >
                            "Continue"
                        </button>
                        <span
                            id="protein-built"
                            class="tree-check"
                            role="status"
                            hidden=move || {
                                !ui.confirmed.get()[Page::Submit as usize] || !ui.has_protein()
                            }
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
                    hidden=move || ui.error(Page::Protein).with(String::is_empty)
                >
                    {move || ui.error(Page::Protein).get()}
                </p>
            </div>
            <ViewerSurface ui panel=Panel::Protein />
        </div>
        <MutationPicker ui modal=mutation query=mutation_query />
        <PtmPicker ui modal=ptm query=ptm_query kind />
        <DisulfidePicker ui modal=ss first second error />
        <glycan::GlycanPicker editor />
    }
}
