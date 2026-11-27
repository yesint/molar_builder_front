use super::*;

pub(super) fn editor(
    ui: Ui,
    residue: RwSignal<u64>,
    residues: StoredValue<Vec<Value>>,
    sequence: RwSignal<String>,
    anomer: RwSignal<String>,
) -> impl IntoView {
    let graph = RwSignal::new(Value::Null);
    let selected = RwSignal::new(0usize);
    let sugar = RwSignal::new("GlcNAc".to_string());
    let linkage = RwSignal::new(4u8);
    let child_anomer = RwSignal::new("b".to_string());
    let edit = move |nodes: Option<Vec<Value>>| {
        let site = residues.with_value(|rows| {
            rows.iter()
                .find(|r| r["id"] == residue.get_untracked())
                .and_then(|r| r["name"].as_str())
                .unwrap_or("")
                .to_owned()
        });
        let body = match nodes {
            Some(nodes) => json!({"site":site,"nodes":nodes}),
            None => {
                json!({"site":site,"iupac":sequence.get_untracked(),"root_anomer":anomer.get_untracked()})
            }
        };
        ui.task(async move {
            let updated: Value = api::post("/api/glycan", &body, None).await?;
            sequence.set(
                updated["iupac"]
                    .as_str()
                    .ok_or("Missing IUPAC sequence")?
                    .to_string(),
            );
            anomer.set(updated["root_anomer"].as_str().unwrap_or("b").to_string());
            graph.set(updated);
            selected.set(0);
            Ok(())
        });
    };
    let nodes =
        move || graph.with_untracked(|g| g["nodes"].as_array().cloned().unwrap_or_default());
    view! {
        <label>
            "Condensed IUPAC sequence"
            <input
                prop:value=move || sequence.get()
                on:input=move |e| {
                    sequence.set(event_target_value(&e));
                    graph.set(Value::Null);
                }
            />
        </label>
        <label>
            "Root anomer"
            <select
                prop:value=move || anomer.get()
                on:change=move |e| {
                    anomer.set(event_target_value(&e));
                    graph.set(Value::Null);
                }
            >
                <option value="b">"β"</option>
                <option value="a">"α"</option>
            </select>
        </label>
        <button on:click=move |_| edit(None)>"Validate and draw glycan"</button>
        <div hidden=move || graph.get().is_null()>
            <img
                alt="SNFG glycan structure"
                src=move || {
                    graph
                        .with(|g| {
                            g["svg"]
                                .as_str()
                                .map(|svg| {
                                    format!(
                                        "data:image/svg+xml;charset=utf-8,{}",
                                        js_sys::encode_uri_component(svg),
                                    )
                                })
                                .unwrap_or_default()
                        })
                }
            />
            <label>
                "Selected sugar"
                <select
                    prop:value=move || selected.get().to_string()
                    on:change=move |e| {
                        selected.set(event_target_value(&e).parse().unwrap_or_default())
                    }
                >
                    {move || {
                        graph
                            .with(|g| {
                                g["nodes"]
                                    .as_array()
                                    .into_iter()
                                    .flatten()
                                    .map(|n| {
                                        view! {
                                            <option value=n["id"]
                                                .to_string()>
                                                {format!(
                                                    "{} {}",
                                                    n["id"],
                                                    n["sugar"].as_str().unwrap_or(""),
                                                )}
                                            </option>
                                        }
                                    })
                                    .collect_view()
                            })
                    }}
                </select>
            </label>
            <label>
                "Sugar"
                <select
                    prop:value=move || sugar.get()
                    on:change=move |e| sugar.set(event_target_value(&e))
                >
                    {move || {
                        graph
                            .with(|g| {
                                g["sugars"]
                                    .as_array()
                                    .into_iter()
                                    .flatten()
                                    .filter_map(Value::as_str)
                                    .map(|s| view! { <option>{s.to_owned()}</option> })
                                    .collect_view()
                            })
                    }}
                </select>
            </label>
            <label>
                "Anomer"<select on:change=move |e| child_anomer.set(event_target_value(&e))>
                    <option value="b">"β"</option>
                    <option value="a">"α"</option>
                </select>
            </label>
            <label>
                "Acceptor position"
                <input
                    type="number"
                    min="1"
                    max="9"
                    value="4"
                    on:input=move |e| linkage.set(event_target_value(&e).parse().unwrap_or(0))
                />
            </label>
            <button on:click=move |_| {
                let mut tree = nodes();
                let id = tree.iter().filter_map(|n| n["id"].as_u64()).max().unwrap_or(0) + 1;
                tree.push(
                    json!(
                        {"id":id,"sugar":sugar.get_untracked(),"anomer":child_anomer.get_untracked(),"parent":selected.get_untracked(),"position":linkage.get_untracked()}
                    ),
                );
                edit(Some(tree));
            }>"Add branch"</button>
            <button on:click=move |_| {
                let mut tree = nodes();
                if let Some(node) = tree.iter_mut().find(|n| n["id"] == selected.get_untracked()) {
                    node["sugar"] = json!(sugar.get_untracked());
                    node["anomer"] = json!(child_anomer.get_untracked());
                    if !node["parent"].is_null() {
                        node["position"] = json!(linkage.get_untracked());
                    }
                }
                edit(Some(tree));
            }>"Update sugar"</button>
            <button on:click=move |_| {
                let mut tree = nodes();
                remove_branch(&mut tree, selected.get_untracked() as u64);
                edit(Some(tree));
            }>"Remove subtree"</button>
        </div>
    }
}

fn remove_branch(tree: &mut Vec<Value>, root: u64) {
    let mut removed = std::collections::BTreeSet::from([root]);
    loop {
        let count = removed.len();
        for node in tree.iter() {
            if node["parent"]
                .as_u64()
                .is_some_and(|parent| removed.contains(&parent))
                && let Some(id) = node["id"].as_u64()
            {
                removed.insert(id);
            }
        }
        if count == removed.len() {
            break;
        }
    }
    tree.retain(|node| !node["id"].as_u64().is_some_and(|id| removed.contains(&id)));
}
