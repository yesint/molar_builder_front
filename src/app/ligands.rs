use super::*;
pub(super) fn descriptions(ui: Ui, p: &Partition) -> Vec<(String, String, String)> {
    let uploads = ui.uploads.get();
    p.ligands
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let info = p.ligand_details.get(i).unwrap_or(&Value::Null);
            let upload = uploads
                .iter()
                .find(|u| Some(u.id.as_str()) == info["source"].as_str());
            let origin = upload.map(|u| u.name.as_str()).unwrap_or(name);
            if let Some(r) = info.get("residue") {
                (
                    r["name"].as_str().unwrap_or(name).to_string(),
                    components::residue_label(r),
                    origin.to_string(),
                )
            } else if let Some((filename, n)) = origin.rsplit_once(" · ") {
                let label = format!("mol {n}");
                (format!("{filename} | {label}"), label, filename.to_owned())
            } else {
                (name.clone(), name.clone(), origin.to_owned())
            }
        })
        .collect()
}
pub(super) fn water(part: &Partition, index: usize) -> bool {
    let Some(residue) = part
        .ligand_details
        .get(index)
        .and_then(|d| d.get("residue"))
    else {
        return false;
    };
    matches!(
        residue["name"].as_str(),
        Some("HOH" | "WAT" | "SOL" | "TIP3" | "TIP3P")
    ) && residue["atoms"] == 3
}
#[derive(Clone)]
struct Entry {
    index: usize,
    label: String,
    origin: String,
    water: bool,
}
fn button(ui: Ui, item: Entry) -> impl IntoView {
    let name = item.label.clone();
    view! {
        <div class="ligand-source-row">
            <button class="secondary compact" on:click=move |_| ui.show_ligand(item.index)>
                {name}
            </button>
            <span class="small">{format!("{} | {}", item.origin, item.label)}</span>
        </div>
    }
}
fn render_group(ui: Ui, name: String, copies: Vec<Entry>) -> impl IntoView {
    let title = format!("{name} ({})", copies.len());
    let origin = copies[0].origin.clone();
    if copies.iter().all(|l| l.water) {
        let details = format!("{origin} | {name}, water ({})", copies.len());
        return view! {
            <div class="ligand-source-row">
                <div>{title}</div>
                <span class="small">{details}</span>
            </div>
        }
        .into_any();
    }
    if copies.len() == 1 {
        return button(ui, copies.into_iter().next().unwrap()).into_any();
    }
    let details = format!("{origin} | {title}");
    view! {
        <details>
            <summary>
                <div class="ligand-source-row">
                    <span>{title}</span>
                    <span class="small">{details}</span>
                </div>
            </summary>
            {paged(copies, move |item| button(ui, item))}
        </details>
    }
    .into_any()
}
fn results(ui: Ui, search: RwSignal<String>) -> impl IntoView {
    move || {
        let Some(part) = ui.part.get() else {
            return ().into_any();
        };
        let filter = search.get();
        let mut groups: Vec<(String, String, Vec<Entry>)> = vec![];
        for (index, (name, label, origin)) in descriptions(ui, &part).into_iter().enumerate() {
            if !name.to_lowercase().contains(&filter) {
                continue;
            }
            let entry = Entry {
                index,
                label,
                origin: origin.clone(),
                water: water(&part, index),
            };
            if let Some((_, _, items)) = groups
                .iter_mut()
                .find(|(o, n, _)| o == &origin && n == &name)
            {
                items.push(entry);
            } else {
                groups.push((origin, name, vec![entry]));
            }
        }
        paged(groups, move |(_, name, list)| render_group(ui, name, list)).into_any()
    }
}
#[component]
pub(super) fn LigandsPage(ui: Ui) -> impl IntoView {
    Effect::new(move |_| {
        if ui.editing.get() {
            ui.set_done(Page::Ligands, false);
            ui.set_done(Page::Submit, false);
        }
    });
    let search = RwSignal::new(String::new());
    let editable = move || ui.has_editable_ligands();
    let save = move |_| {
        ui.task(async move {
            ui.viewer.save().await?;
            ui.viewer.send(Action::Edit(false));
            ui.set_done(Page::Ligands, true);
            ui.go(Page::Membrane);
            Ok(())
        })
    };
    let download = move |e: leptos::ev::MouseEvent| {
        e.prevent_default();
        let i = ui.ligand.get_untracked();
        if let Some(bytes) = ui.result.with_untracked(|r| r.as_ref()?.ligand_bytes(i)) {
            spawn_local(async move {
                if let Err(e) =
                    checkout::save_file(&format!("ligand-{}-parameters.zip", i + 1), &bytes).await
                {
                    ui.msg.set(e);
                }
            });
        }
    };
    view! {
        <StageHeading ui page=Page::Ligands />
        <p class="small">
            "All ligand hydrogens must be present; missing hydrogens will cause parameterization to fail."
        </p>
        <div class="protein-layout">
            <div class="protein-controls">
                <label hidden=move || {
                    !ui.has_ligands()
                }>
                    "Search by resname "
                    <input
                        id="ligand-resname-search"
                        type="search"
                        on:input=move |e| search.set(event_target_value(&e).to_lowercase())
                    />
                </label>
                <div id="prepared-ligands">{results(ui, search)}</div>
                <div class="actions">
                    <button
                        id="edit-ligand"
                        class="secondary"
                        type="button"
                        disabled=move || !editable() || ui.busy.get()
                        on:click=move |_| ui.viewer.send(Action::Edit(!ui.editing.get_untracked()))
                    >
                        {move || if ui.editing.get() { "Done" } else { "Edit" }}
                    </button>
                    <button
                        id="parameterize-ligand"
                        type="button"
                        disabled=move || !ui.has_ligands() || ui.busy.get()
                        on:click=save
                    >
                        "Continue"
                    </button>
                    <a
                        id="ligand-download"
                        href="#"
                        hidden=move || {
                            !ui
                                .result
                                .with(|r| r.as_ref().is_some_and(|r| r.has_ligand(ui.ligand.get())))
                        }
                        on:click=download
                    >
                        "Download parameters"
                    </a>
                </div>
                <div id="ligand-progress" role="status" hidden>
                    <progress></progress>
                    " Parameterizing…"
                </div>
                <p
                    id="ligand-error"
                    class="error"
                    role="alert"
                    hidden=move || ui.msg.with(String::is_empty)
                >
                    {move || ui.msg.get()}
                </p>
            </div>
            <ViewerSurface ui panel=Panel::Ligand />
        </div>
    }
}
