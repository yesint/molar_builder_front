use super::*;
use crate::viewer::{Layer, Preview};
use std::collections::BTreeSet;

#[derive(Clone)]
pub(super) struct Source {
    name: String,
    extra: bool,
    models: Vec<Model>,
}
#[derive(Clone)]
struct Model {
    upload: Upload,
    groups: [Vec<Group>; 2],
    selected: [RwSignal<BTreeSet<String>>; 2],
    custom: [RwSignal<Vec<String>>; 2],
}
#[derive(Clone)]
struct Group {
    label: String,
    keys: Vec<String>,
    residues: Vec<Value>,
}
fn items<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v[key].as_array().map(Vec::as_slice).unwrap_or_default()
}
pub(super) fn residue_label(r: &Value) -> String {
    let chain = r["chain"].as_str().unwrap_or("").trim();
    format!(
        "{} {}{}",
        r["name"].as_str().unwrap_or(""),
        if chain.is_empty() {
            String::new()
        } else {
            format!("{chain}:")
        },
        r["number"]
    )
}
impl Model {
    fn new(upload: Upload, index: usize, extra: bool) -> Self {
        let report = &upload.report;
        let mut groups = [vec![], vec![]];
        let mut selected = [BTreeSet::new(), BTreeSet::new()];
        for chain in items(report, "chains") {
            let key = chain["id"].as_str().unwrap_or("").to_owned();
            if !extra && index == 0 {
                selected[0].insert(key.clone());
            }
            groups[0].push(Group {
                label: if key.trim().is_empty() {
                    "No chain ID".into()
                } else {
                    format!("Chain {key}")
                },
                keys: vec![key],
                residues: vec![],
            });
        }
        if upload.format == "sdf" {
            groups[1].push(Group {
                label: format!("mol {}", index + 1),
                keys: vec!["all".into()],
                residues: vec![],
            });
            selected[1].insert("all".into());
        }
        for category in ["ligands", "other"] {
            let rows = items(report, category);
            let default = category == "ligands"
                || (rows.len() == 1
                    && ["chains", "ligands", "environment"]
                        .iter()
                        .all(|k| items(report, k).is_empty()));
            let mut names = vec![];
            for r in rows {
                if names.contains(&&r["name"]) {
                    continue;
                }
                names.push(&r["name"]);
                let residues: Vec<_> = rows
                    .iter()
                    .filter(|x| x["name"] == r["name"])
                    .cloned()
                    .collect();
                let keys: Vec<_> = residues.iter().map(|v| v["id"].to_string()).collect();
                if default && index == 0 {
                    selected[1].extend(keys.iter().cloned());
                }
                let label = if residues.len() == 1 {
                    format!("{} (resindex {})", residue_label(r), r["id"])
                } else {
                    format!("{} ({})", r["name"].as_str().unwrap_or(""), residues.len())
                };
                groups[1].push(Group {
                    label,
                    keys,
                    residues,
                });
            }
        }
        for env in items(report, "environment") {
            let name = env["name"].as_str().unwrap_or("");
            groups[1].push(Group {
                label: format!(
                    "{name}, {} ({})",
                    env["category"].as_str().unwrap_or("").replace('_', " "),
                    env["residues"]
                ),
                keys: vec![name.into()],
                residues: vec![],
            });
        }
        Self {
            upload,
            groups,
            selected: selected.map(RwSignal::new),
            custom: std::array::from_fn(|_| RwSignal::new(vec![])),
        }
    }
    fn expression(&self, role: usize) -> String {
        let selected = self.selected[role].get();
        let report = &self.upload.report;
        let mut parts = vec![];
        if !selected.is_empty() {
            if self.upload.format == "sdf" {
                parts.push("all".to_owned());
            } else if role == 0
                && selected.len() == items(report, "chains").len()
                && let Some(sel) = report["protein_selection"].as_str()
            {
                parts.push(sel.into());
            } else {
                let rows = items(report, "residues");
                let ids: Vec<_> = rows
                    .iter()
                    .filter(|r| {
                        if role == 0 {
                            r["protein"] == true
                                && selected.contains(r["chain"].as_str().unwrap_or(""))
                        } else {
                            selected.contains(&r["id"].to_string())
                        }
                    })
                    .map(|r| {
                        r["preview_index"]
                            .as_u64()
                            .or(r["id"].as_u64())
                            .unwrap()
                            .to_string()
                    })
                    .collect();
                if !ids.is_empty() {
                    parts.push(format!("resindex {}", ids.join(" ")));
                }
                if role == 1 {
                    let names: Vec<_> = items(report, "environment")
                        .iter()
                        .filter_map(|e| e["name"].as_str())
                        .filter(|s| selected.contains(*s))
                        .collect();
                    if !names.is_empty() {
                        let except = rows
                            .iter()
                            .filter(|r| names.contains(&r["name"].as_str().unwrap_or("")))
                            .filter_map(|r| r["preview_index"].as_u64().or(r["id"].as_u64()))
                            .map(|n| n.to_string())
                            .collect::<Vec<_>>();
                        let group = format!("resname {}", names.join(" "));
                        parts.push(if except.is_empty() {
                            group
                        } else {
                            format!("({group}) and not (resindex {})", except.join(" "))
                        });
                    }
                }
            }
        }
        parts.extend(self.custom[role].get());
        if parts.len() < 2 {
            parts.pop().unwrap_or_default()
        } else {
            parts
                .into_iter()
                .map(|s| format!("({s})"))
                .collect::<Vec<_>>()
                .join(" or ")
        }
    }
}
pub(super) fn preview(ui: Ui) {
    let models = ui.choices.with(|choices| {
        ui.files.with_untracked(|sources| {
            let mut primary = true;
            let models = sources.iter().flat_map(|source| &source.models);
            models
                .zip(choices)
                .map(|(model, selection)| {
                    let upload = &model.upload;
                    let first_structure = primary && upload.format != "sdf";
                    if first_structure {
                        primary = false;
                    }
                    let file = if upload.format == "sdf" {
                        "source.sdf"
                    } else {
                        "structure.cif"
                    };
                    let protein_style = if upload.report["coarse_grained"] == true {
                        "vdw"
                    } else {
                        "cartoon"
                    };
                    let ligand_style =
                        if first_structure && !items(&upload.report, "chains").is_empty() {
                            "licorice"
                        } else {
                            "ballstick"
                        };
                    Preview {
                        bytes: None,
                        path: format!("/api/file/upload/{}/{file}", upload.id),
                        name: file.into(),
                        layers: vec![
                            Layer::new(
                                "protein",
                                selection.protein_selection.clone(),
                                protein_style,
                                "chain",
                            ),
                            Layer::new(
                                "ligands",
                                selection.ligand_selection.clone(),
                                ligand_style,
                                "element",
                            ),
                        ],
                    }
                })
                .collect()
        })
    });
    ui.viewer.send(Action::Preview("load-viewer", models));
}
fn checkbox(
    ui: Ui,
    label: String,
    targets: Vec<(RwSignal<BTreeSet<String>>, Vec<String>)>,
) -> impl IntoView {
    let targets = StoredValue::new(targets);
    let count = move || {
        targets.with_value(|v| {
            v.iter().fold((0, 0), |(n, t), (s, keys)| {
                (
                    n + s.with(|set| keys.iter().filter(|key| set.contains(*key)).count()),
                    t + keys.len(),
                )
            })
        })
    };
    view! {
        <label class="protein-check">
            <input
                type="checkbox"
                prop:checked=move || {
                    let (n, t) = count();
                    n == t && t > 0
                }
                prop:indeterminate=move || {
                    let (n, t) = count();
                    n > 0 && n < t
                }
                on:change=move |e| {
                    let check = event_target_checked(&e);
                    targets
                        .with_value(|targets| {
                            for (s, keys) in targets {
                                s.update(|set| {
                                    for key in keys {
                                        if check {
                                            set.insert(key.clone());
                                        } else {
                                            set.remove(key);
                                        }
                                    }
                                })
                            }
                        });
                    ui.invalidate_components();
                }
            />
            {label}
        </label>
    }
}
fn group(ui: Ui, source: Source, role: usize) -> impl IntoView {
    let all = source
        .models
        .iter()
        .map(|m| {
            (
                m.selected[role],
                m.groups[role]
                    .iter()
                    .flat_map(|g| g.keys.clone())
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    if all.iter().all(|(_, k)| k.is_empty()) {
        return ().into_any();
    }
    let multiple = source.models.len() > 1;
    let mut choices = Vec::new();
    for (index, model) in source.models.iter().enumerate() {
        for group in &model.groups[role] {
            let name = if multiple && model.upload.format != "sdf" {
                format!("model {} · {}", index + 1, group.label)
            } else {
                group.label.clone()
            };
            choices.push((name, model.selected[role], group.clone()));
        }
    }
    let render = move |(name, selected, group): (String, RwSignal<BTreeSet<String>>, Group)| {
        let control = checkbox(ui, name, vec![(selected, group.keys)]);
        if group.residues.len() < 2 {
            return control.into_any();
        }
        let children = paged(group.residues, move |r| {
            let label = format!("{} (resindex {})", residue_label(&r), r["id"]);
            checkbox(ui, label, vec![(selected, vec![r["id"].to_string()])])
        });
        view! {
            <div>
                {control}<details>
                    <summary>"Residues"</summary>
                    {children}
                </details>
            </div>
        }
        .into_any()
    };
    view! {
        <div class="component-source">
            <h4>{source.name}</h4>
            {checkbox(ui, "All".into(), all)}
            {paged(choices, render)}
        </div>
    }
    .into_any()
}
fn custom(ui: Ui, source: Source, role: usize) -> impl IntoView {
    let models = StoredValue::new(source.models);
    let active = RwSignal::new(0usize);
    let drafts = StoredValue::new(models.with_value(|rows| {
        rows.iter()
            .map(|_| RwSignal::new(String::new()))
            .collect::<Vec<_>>()
    }));
    let input = NodeRef::<leptos::html::Input>::new();
    let draft = move || drafts.with_value(|v| v[active.get()]);
    let add = move || {
        let value = draft().get_untracked().trim().to_string();
        if value.is_empty() {
            return;
        }
        models.with_value(|v| {
            v[active.get_untracked()].custom[role].update(|list| {
                if !list.contains(&value) {
                    list.push(value);
                }
            })
        });
        draft().set(String::new());
        if let Some(input) = input.get_untracked() {
            let _ = input.focus();
        }
        ui.invalidate_components();
    };
    let id = format!(
        "{}-selection-{}",
        ["protein", "ligand"][role],
        models.with_value(|m| m[0].upload.id.clone())
    );
    view! {
        <div class="custom-source">
            <div class="selection-entry" class:with-model=models.with_value(|v| v.len() > 1)>
                <label class="selection-source" for=id.clone()>
                    {source.name.clone()}
                </label>
                {models
                    .with_value(|ms| {
                        (ms.len() > 1)
                            .then(|| {
                                view! {
                                    <select on:change=move |e| {
                                        active.set(event_target_value(&e).parse().unwrap_or(0))
                                    }>
                                        {ms
                                            .iter()
                                            .enumerate()
                                            .map(|(i, m)| {
                                                view! {
                                                    <option value=i>
                                                        {format!(
                                                            "{} {}",
                                                            if m.upload.format == "sdf" { "mol" } else { "model" },
                                                            i + 1,
                                                        )}
                                                    </option>
                                                }
                                            })
                                            .collect_view()}
                                    </select>
                                }
                            })
                    })}
                <input
                    id=id
                    node_ref=input
                    type="text"
                    autocomplete="off"
                    spellcheck="false"
                    prop:value=move || draft().get()
                    on:input=move |e| draft().set(event_target_value(&e))
                    on:keydown=move |e| {
                        if e.key() == "Enter" {
                            e.prevent_default();
                            add();
                        }
                    }
                />
                <button
                    class="secondary compact"
                    aria-label=format!(
                        "Add {} selection for {}",
                        ["protein", "ligand"][role],
                        source.name,
                    )
                    disabled=move || draft().get().trim().is_empty()
                    on:click=move |_| add()
                >
                    "+"
                </button>
            </div>
            <ul class="saved-selections">
                {move || {
                    models
                        .with_value(|ms| {
                            ms.iter()
                                .flat_map(|m| {
                                    let saved = m.custom[role];
                                    saved
                                        .get()
                                        .into_iter()
                                        .enumerate()
                                        .map(move |(i, item)| {
                                            view! {
                                                <li>
                                                    <input
                                                        prop:value=item
                                                        on:change=move |e| {
                                                            let input = event_target::<HtmlInputElement>(&e);
                                                            let value = input.value().trim().to_owned();
                                                            if value.is_empty() {
                                                                saved.with_untracked(|v| input.set_value(&v[i]));
                                                                return;
                                                            }
                                                            saved.update(|v| v[i] = value);
                                                            ui.invalidate_components();
                                                        }
                                                    />
                                                    <button
                                                        class="secondary compact"
                                                        on:click=move |_| {
                                                            saved
                                                                .update(|v| {
                                                                    v.remove(i);
                                                                });
                                                            ui.invalidate_components();
                                                        }
                                                    >
                                                        "×"
                                                    </button>
                                                </li>
                                            }
                                        })
                                })
                                .collect_view()
                        })
                }}
            </ul>
        </div>
    }
}
#[component]
fn FilePicker(ui: Ui, extra: bool, status: RwSignal<&'static str>) -> impl IntoView {
    let input = NodeRef::<leptos::html::Input>::new();
    let (button, picker, names, title) = if extra {
        (
            "add-ligand-files",
            "ligand-files",
            "ligand-filenames",
            "Add ligands...",
        )
    } else {
        (
            "load-structure",
            "structure-file",
            "structure-filename",
            "Load...",
        )
    };
    let upload = move |_| {
        let Some(input) = input.get_untracked() else {
            return;
        };
        let Some(files) = input.files() else { return };
        let files: Vec<_> = (0..files.length()).filter_map(|i| files.item(i)).collect();
        input.set_value("");
        if files.is_empty() {
            return;
        }
        status.set("Reading components…");
        ui.task(async move {
            let mut batch = Vec::new();
            for file in files {
                let name = file.name();
                let models = api::upload(&file)
                    .await?
                    .into_iter()
                    .enumerate()
                    .map(|(i, u)| Model::new(u, i, extra))
                    .collect();
                batch.push(Source {
                    name,
                    extra,
                    models,
                });
            }
            ui.files.update(|v| v.extend(batch));
            ui.invalidate_components();
            Ok(())
        });
    };
    view! {
        <button
            type="button"
            id=button
            class:secondary=extra
            disabled=move || ui.busy.get()
            on:click=move |_| {
                if let Some(input) = input.get_untracked() {
                    input.click();
                }
            }
        >
            {title}
        </button>
        <input
            id=picker
            node_ref=input
            type="file"
            accept=".pdb,.cif,.mmcif,.sdf,.sd"
            multiple
            hidden
            on:change=upload
        />
        <span id=names class="small">
            {move || {
                ui.files
                    .get()
                    .into_iter()
                    .enumerate()
                    .filter(|(_, s)| s.extra == extra)
                    .map(|(i, s)| {
                        view! {
                            <span class="component-filename">
                                {s.name.clone()}
                                <button
                                    class="secondary compact"
                                    aria-label=format!("Remove {}", s.name)
                                    disabled=move || ui.busy.get()
                                    on:click=move |_| {
                                        ui.files
                                            .update(|v| {
                                                v.remove(i);
                                            });
                                        ui.invalidate_components();
                                    }
                                >
                                    "×"
                                </button>
                            </span>
                        }
                    })
                    .collect_view()
            }}
        </span>
    }
}

#[component]
fn SelectionTabs(ui: Ui, role: usize) -> impl IntoView {
    let protein = role == 0;
    let (title, list, list_tab, custom_tab, list_panel, custom_panel, host) = if protein {
        (
            "Protein",
            "Protein selection",
            "protein-chains-tab",
            "protein-custom-tab",
            "protein-chains-panel",
            "protein-custom-panel",
            "load-chains",
        )
    } else {
        (
            "Ligands",
            "Ligand selection",
            "ligand-residues-tab",
            "ligand-custom-tab",
            "ligand-residues-panel",
            "ligand-custom-panel",
            "load-ligands",
        )
    };
    let active = RwSignal::new(false);
    let refs = [NodeRef::<leptos::html::Button>::new(), NodeRef::new()];
    let tab = move |is_custom: bool| {
        let (id, panel, title) = if is_custom {
            (custom_tab, custom_panel, "Custom selection")
        } else {
            (list_tab, list_panel, "Choose")
        };
        view! {
            <button
                id=id
                node_ref=refs[usize::from(is_custom)]
                type="button"
                role="tab"
                aria-controls=panel
                aria-selected=move || (active.get() == is_custom).to_string()
                tabindex=move || if active.get() == is_custom { "0" } else { "-1" }
                on:click=move |_| active.set(is_custom)
                on:keydown=move |e| {
                    let next = match e.key().as_str() {
                        "Home" => false,
                        "End" => true,
                        "ArrowLeft" | "ArrowRight" => !is_custom,
                        _ => return,
                    };
                    e.prevent_default();
                    active.set(next);
                    if let Some(b) = refs[usize::from(next)].get_untracked() {
                        let _ = b.focus();
                    }
                }
            >
                {title}
            </button>
        }
    };
    view! {
        <div class="protein-section" id=protein.then_some("load-protein-section")>
            <h3>{title}</h3>
            <div class="component-tabs" role="tablist" aria-label=list>
                {tab(false)}
                {tab(true)}
            </div>
            <div id=list_panel role="tabpanel" aria-labelledby=list_tab hidden=move || active.get()>
                <div id=host class:component-ligand-list=!protein>
                    {move || ui.files.get().into_iter().map(|s| group(ui, s, role)).collect_view()}
                </div>
            </div>
            <div
                id=custom_panel
                role="tabpanel"
                aria-labelledby=custom_tab
                hidden=move || !active.get()
            >
                {move || ui.files.get().into_iter().map(|s| custom(ui, s, role)).collect_view()}
            </div>
        </div>
    }
}

#[component]
pub(super) fn ComponentsPage(ui: Ui) -> impl IntoView {
    Effect::new(move |_| {
        let files = ui.files.get();
        let project = files
            .iter()
            .find(|s| !s.extra)
            .and_then(|s| std::path::Path::new(&s.name).file_stem())
            .and_then(|s| s.to_str())
            .map(|s| {
                s.chars()
                    .map(|ch| {
                        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '.' | '_') {
                            ch
                        } else {
                            '_'
                        }
                    })
                    .take(120)
                    .collect::<String>()
            })
            .filter(|s| s.starts_with(|ch: char| ch.is_ascii_alphanumeric()))
            .unwrap_or_else(|| "project".into());
        if ui.plan.with_untracked(|p| p.project != project) {
            ui.plan.update(|p| p.project = project);
        }
        let models = files.iter().flat_map(|s| &s.models);
        ui.uploads
            .set(models.clone().map(|m| m.upload.clone()).collect());
    });
    Effect::new(move |_| {
        ui.files.with(|files| {
            ui.choices.set(
                files
                    .iter()
                    .flat_map(|s| &s.models)
                    .map(|m| Selection {
                        id: m.upload.id.clone(),
                        protein_selection: m.expression(0),
                        ligand_selection: m.expression(1),
                    })
                    .collect(),
            );
        });
    });
    let status = RwSignal::new("");
    let empty = move || ui.files.with(Vec::is_empty);
    let disabled = move || empty() || ui.busy.get();
    let idle = move || !ui.busy.get() || ui.stage.get() != 0;
    let partition = move |_| {
        status.set("Partitioning system…");
        ui.task(async move {
            ui.partition_components().await?;
            ui.set_done(0, true);
            ui.go(if ui.has_protein() {
                1
            } else if ui.has_ligands() {
                2
            } else {
                3
            });
            Ok(())
        });
    };
    view! {
        <div class="stage-heading">
            <h2 id="load-heading" tabindex="-1">
                "1. Components"
            </h2>
            <button
                class="secondary help-button"
                type="button"
                aria-label="Help for Components"
                aria-haspopup="dialog"
                aria-controls="stage-help"
                data-help="Load your files and choose which proteins and other molecules to keep. If you only want a membrane, go straight to the Membrane step."
            >
                "?"
            </button>
        </div>
        <div class="protein-layout">
            <div class="protein-controls">
                <div class="protein-toolbar component-files">
                    <FilePicker ui extra=false status />
                    <FilePicker ui extra=true status />
                </div>
                <fieldset id="load-options" hidden=empty disabled=disabled>
                    <SelectionTabs ui role=0 />
                    <SelectionTabs ui role=1 />
                </fieldset>
                <div class="actions" id="load-actions" hidden=empty>
                    <button
                        id="partition-system"
                        type="button"
                        disabled=disabled
                        on:click=partition
                    >
                        "Continue"
                    </button>
                </div>
                <p id="load-status" role="status" hidden=idle>
                    {move || status.get()}
                </p>
                <progress id="load-progress" hidden=idle></progress>
                <p
                    id="load-error"
                    class="error"
                    role="alert"
                    hidden=move || ui.msg.with(String::is_empty)
                >
                    {move || ui.msg.get()}
                </p>
            </div>
            <div
                id="load-viewer"
                hidden=empty
                class="structure-viewer"
                aria-label="Loaded structure"
            ></div>
        </div>
    }
}
