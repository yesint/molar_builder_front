use super::*;

pub(super) fn protein(ui: Ui) -> impl IntoView {
    let Some(p) = ui.part.get_untracked().filter(|p| !p.protein.is_null()) else {
        return view! { <p>"No protein selected."</p> }.into_any();
    };
    let residues = p.protein["residues"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let residue = RwSignal::new(residues.first().and_then(|r| r["id"].as_u64()).unwrap_or(0));
    let target = RwSignal::new("ALA".to_string());
    let kind = RwSignal::new("phosphorylation".to_string());
    let iupac = RwSignal::new("GlcNAc".to_string());
    let charge = RwSignal::new(-2i8);
    let anomer = RwSignal::new("b".to_string());
    let inventory = StoredValue::new(residues.clone());
    let possible_bonds = p.protein["ss_bonds"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let names = "ALA ARG ASN ASP CYS GLN GLU GLY HIS ILE LEU LYS MET PHE PRO SER THR TRP TYR VAL";
    view! {
        <label>
            <input
                type="checkbox"
                prop:checked=move || ui.plan.get().protein.orient
                on:change=move |e| ui.plan.update(|p| p.protein.orient = input(&e).checked())
            />
            "Orient protein in membrane"
        </label>
        <label>
            "Residue"
            <select on:change=move |e| {
                residue.set(event_target_value(&e).parse().unwrap_or_default())
            }>
                {residues
                    .iter()
                    .map(|r| {
                        view! {
                            <option value=r["id"]
                                .to_string()>
                                {format!(
                                    "{} {} {}",
                                    r["chain"].as_str().unwrap_or(""),
                                    r["name"].as_str().unwrap_or(""),
                                    r["number"],
                                )}
                            </option>
                        }
                    })
                    .collect_view()}
            </select>
        </label>
        <h2>"Mutation"</h2>
        <select
            aria-label="Replacement residue"
            on:change=move |e| target.set(event_target_value(&e))
        >
            {names.split_whitespace().map(|n| view! { <option>{n}</option> }).collect_view()}
        </select>
        <button on:click=move |_| {
            ui.plan
                .update(|p| {
                    let id = residue.get_untracked();
                    p.protein.mutations.retain(|v| v["residue"] != id);
                    p.protein.mutations.push(json!({"residue":id,"target":target.get_untracked()}))
                })
        }>"Add mutation"</button>
        <h2>"Post-translational modifications"</h2>
        <select aria-label="Modification" on:change=move |e| kind.set(event_target_value(&e))>
            {[
                "phosphorylation",
                "palmitoylation",
                "glycine_myristoylation",
                "lysine_myristoylation",
                "farnesylation",
                "geranylgeranylation",
                "triacylation",
                "glycosylation",
            ]
                .into_iter()
                .map(|k| view! { <option>{k}</option> })
                .collect_view()}
        </select>
        <label hidden=move || {
            kind.get() != "phosphorylation"
        }>
            "Phosphate charge"
            <select on:change=move |e| charge.set(event_target_value(&e).parse().unwrap_or(-2))>
                <option>"-2"</option>
                <option>"-1"</option>
            </select>
        </label>
        <section hidden=move || {
            kind.get() != "glycosylation"
        }>{super::glycan::editor(ui, residue, inventory, iupac, anomer)}</section>
        <button
            disabled=move || {
                let selected = residue.get();
                let modification = kind.get();
                !ui
                    .plan
                    .with(|plan| {
                        inventory
                            .with_value(|rows| can_modify(
                                &plan.protein,
                                rows,
                                selected,
                                &modification,
                            ))
                    })
            }
            on:click=move |_| {
                ui.plan
                    .update(|p| {
                        let k = kind.get_untracked();
                        let mut v = json!({"residue":residue.get_untracked(),"type":k});
                        if k == "phosphorylation" {
                            v["charge"] = json!(charge.get_untracked());
                        }
                        if k == "glycosylation" {
                            v["iupac"] = json!(iupac.get_untracked());
                            v["root_anomer"] = json!(anomer.get_untracked());
                        }
                        p.protein.ptms.push(v);
                    })
            }
        >
            "Add modification"
        </button>
        <h2>"Disulfide bonds"</h2>
        {possible_bonds
            .into_iter()
            .filter_map(|candidate| {
                let pair: [usize; 2] = serde_json::from_value(candidate["residues"].clone()).ok()?;
                let labels = pair
                    .map(|id| {
                        inventory
                            .with_value(|rows| {
                                rows.iter()
                                    .find(|r| r["id"] == id)
                                    .map(|r| {
                                        format!(
                                            "{} {}",
                                            r["chain"].as_str().unwrap_or(""),
                                            r["number"],
                                        )
                                    })
                                    .unwrap_or_default()
                            })
                    });
                let distance = candidate["distance_nm"].as_f64()?;
                Some(
                    view! {
                        <label>
                            <input
                                type="checkbox"
                                prop:checked=move || {
                                    ui.plan.with(|p| p.protein.ss_bonds.contains(&pair))
                                }
                                on:change=move |event| {
                                    ui.plan
                                        .update(|p| {
                                            p.protein.ss_bonds.retain(|entry| entry != &pair);
                                            if input(&event).checked() {
                                                p.protein.ss_bonds.push(pair);
                                            }
                                        })
                                }
                            />
                            {format!("Cys {} – Cys {} ({distance:.3} nm)", labels[0], labels[1])}
                        </label>
                    },
                )
            })
            .collect_view()}
        {move || {
            let p = ui.plan.get().protein;
            [
                ("Mutation", p.mutations),
                ("Modification", p.ptms),
                ("Disulfide", p.ss_bonds.iter().map(|b| json!(b)).collect()),
            ]
                .into_iter()
                .flat_map(move |(k, items)| {
                    items
                        .into_iter()
                        .enumerate()
                        .map(move |(i, v)| {
                            view! {
                                <p>
                                    {format!("{k} {v}")}" "
                                    <button on:click=move |_| {
                                        ui.plan
                                            .update(|p| match k {
                                                "Mutation" => {
                                                    p.protein.mutations.remove(i);
                                                }
                                                "Modification" => {
                                                    p.protein.ptms.remove(i);
                                                }
                                                _ => {
                                                    p.protein.ss_bonds.remove(i);
                                                }
                                            })
                                    }>"Remove"</button>
                                </p>
                            }
                        })
                })
                .collect_view()
        }}
    }
    .into_any()
}

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
