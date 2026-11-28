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
pub(super) fn setup(ui: Ui) {
    let search = RwSignal::new(String::new());
    on("ligand-resname-search", "input", move |_| {
        search.set(field("ligand-resname-search").value().to_lowercase())
    });
    mount("prepared-ligands", move || {
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
    });
    click("edit-ligand", move || {
        ui.viewer.send(Action::Edit(!ui.editing.get_untracked()))
    });
    click("parameterize-ligand", move || {
        ui.task(async move {
            ui.viewer.save().await?;
            ui.viewer.send(Action::Edit(false));
            ui.set_done(2, true);
            ui.go(3);
            Ok(())
        })
    });
    Effect::new(move |_| {
        let has = ui.has_ligands();
        let editable = ui.part.with(|p| {
            p.as_ref()
                .is_some_and(|p| p.ligands.iter().enumerate().any(|(i, _)| !water(p, i)))
        });
        visible("ligand-viewer", editable);
        let limit = ui.part.with(|p| p.as_ref().map_or(0, |p| p.ligands.len()));
        el("ligand-resname-search")
            .parent_element()
            .unwrap()
            .toggle_attribute_with_force("hidden", limit == 0)
            .unwrap();
        enabled("parameterize-ligand", has && !ui.busy.get());
        enabled("edit-ligand", editable && !ui.busy.get());
        label(
            "edit-ligand",
            if ui.editing.get() { "Done" } else { "Edit" },
        );
    });
}
