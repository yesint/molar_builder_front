use super::*;

pub(super) fn components(ui: Ui) -> impl IntoView {
    view! {
        <label>
            "Upload structures"
            <input
                multiple
                type="file"
                accept=".pdb,.cif,.mmcif,.sd,.sdf"
                on:change=move |event| {
                    let field = input(&event);
                    if let Some(list) = field.files() {
                        let files = (0..list.length())
                            .filter_map(|index| list.item(index))
                            .collect();
                        ui.task(load_files(ui, files));
                    }
                    field.set_value("");
                }
            />
        </label>
        <p>"For a membrane-only system, continue without uploading a file."</p>
        <For
            each=move || ui.uploads.get()
            key=|upload| upload.id.clone()
            children=move |upload| source_card(ui, upload)
        />
    }
}

async fn load_files(ui: Ui, files: Vec<web_sys::File>) -> Result<(), String> {
    for file in files {
        for upload in api::upload(&file).await? {
            let protein = if upload.report["chains"]
                .as_array()
                .is_some_and(|chains| !chains.is_empty())
            {
                upload.report["protein_selection"]
                    .as_str()
                    .unwrap_or_default()
            } else {
                ""
            };
            let residues = upload.report["ligands"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|residue| residue["preview_index"].as_u64())
                .map(|index| index.to_string())
                .collect::<Vec<_>>();
            let ligand = if upload.format == "sdf" {
                "all".into()
            } else if residues.is_empty() {
                String::new()
            } else {
                format!("resindex {}", residues.join(" "))
            };
            ui.choices.update(|choices| {
                choices.push(Selection {
                    id: upload.id.clone(),
                    protein_selection: protein.into(),
                    ligand_selection: ligand,
                })
            });
            ui.uploads.update(|uploads| uploads.push(upload));
            ui.invalidate_components();
        }
    }
    ui.preview(0);
    Ok(())
}

fn source_card(ui: Ui, upload: Upload) -> impl IntoView {
    let id = StoredValue::new(upload.id.clone());
    let preview = upload.clone();
    let chosen = move |protein: bool| {
        ui.choices.with(|choices| {
            choices
                .iter()
                .find(|choice| choice.id == id.get_value())
                .map(|choice| {
                    if protein {
                        choice.protein_selection.clone()
                    } else {
                        choice.ligand_selection.clone()
                    }
                })
                .unwrap_or_default()
        })
    };
    let choose =
        move |protein: bool, value: String| ui.change_choices(&id.get_value(), protein, value);
    let preset = move |protein: bool, expression: String| {
        let previous = chosen(protein);
        let combined = if previous.is_empty() {
            expression
        } else {
            format!("({previous}) or ({expression})")
        };
        choose(protein, combined);
    };
    let query = RwSignal::new(String::new());
    let residues = StoredValue::new(
        upload.report["residues"]
            .as_array()
            .cloned()
            .unwrap_or_default(),
    );
    let chains = upload.report["chains"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|chain| chain["id"].as_str())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let empty = chains.is_empty() && residues.with_value(Vec::is_empty);
    view! {
        <div class="card">
            <button on:click=move |_| ui.preview_upload(&preview)>{upload.name}</button>
            <button on:click=move |_| {
                ui.uploads.update(|uploads| uploads.retain(|upload| upload.id != id.get_value()));
                ui.choices.update(|choices| choices.retain(|choice| choice.id != id.get_value()));
                ui.invalidate_components();
                ui.preview(0);
            }>"Remove file"</button>
            <label>
                "Protein atom selection"
                <input
                    prop:value=move || chosen(true)
                    on:change=move |event| choose(true, event_target_value(&event))
                />
            </label>
            <label>
                "Ligand atom selection"
                <input
                    prop:value=move || chosen(false)
                    on:change=move |event| choose(false, event_target_value(&event))
                />
            </label>
            <details hidden=empty>
                <summary>"Choose chains or residues"</summary>
                {chains
                    .into_iter()
                    .map(|chain| {
                        let expression = format!("chain {chain}");
                        view! {
                            <button on:click=move |_| preset(
                                true,
                                expression.clone(),
                            )>{format!("Add chain {chain} to protein")}</button>
                        }
                    })
                    .collect_view()}
                <label>
                    "Find a residue"
                    <input
                        type="search"
                        on:input=move |e| query.set(event_target_value(&e).to_lowercase())
                    />
                </label>
                <div class="picker">
                    {move || {
                        let search = query.get();
                        residues
                            .with_value(|rows| {
                                rows.iter()
                                    .filter_map(|r| {
                                        let label = format!(
                                            "{} {} {}",
                                            r["chain"].as_str().unwrap_or(""),
                                            r["name"].as_str()?,
                                            r["number"],
                                        );
                                        label
                                            .to_lowercase()
                                            .contains(&search)
                                            .then_some((label, r["preview_index"].as_u64()?))
                                    })
                                    .take(100)
                                    .map(|(label, index)| {
                                        view! {
                                            <div>
                                                {label}" "
                                                <button on:click=move |_| preset(
                                                    true,
                                                    format!("resindex {index}"),
                                                )>"Add to protein"</button>
                                                <button on:click=move |_| preset(
                                                    false,
                                                    format!("resindex {index}"),
                                                )>"Add to ligands"</button>
                                            </div>
                                        }
                                    })
                                    .collect_view()
                            })
                    }}
                </div>
            </details>
        </div>
    }
}
