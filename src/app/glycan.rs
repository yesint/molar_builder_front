use super::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
#[derive(Clone, PartialEq, Deserialize, Serialize)]
struct Sugar {
    id: u64,
    parent: Option<u64>,
    position: Option<u8>,
    sugar: String,
    anomer: String,
}
#[derive(Clone, Default, Deserialize)]
struct Graph {
    iupac: String,
    formatted_iupac: String,
    root_anomer: String,
    nodes: Vec<Sugar>,
    svg: String,
    sugars: Vec<String>,
    donors: HashMap<String, u8>,
    acceptors: HashMap<u64, Vec<u8>>,
    linkage_labels: HashMap<String, String>,
    id_map: HashMap<u64, u64>,
}
#[derive(Clone, Copy)]
pub(super) struct Editor {
    ui: Ui,
    modal: Modal,
    site: RwSignal<u64>,
    graph: RwSignal<Graph>,
    selected: RwSignal<u64>,
    history: RwSignal<Vec<Vec<Sugar>>>,
    generation: StoredValue<u64>,
    text: RwSignal<String>,
    error: RwSignal<String>,
    pending: RwSignal<bool>,
    valid: RwSignal<bool>,
    tree: NodeRef<leptos::html::Div>,
}
enum Change {
    Sugar(String),
    Anomer(String),
    Position(u8),
    Remove,
    Add(String, String, u8),
}
fn attachment_label(residue: &Value, anomer: &str) -> String {
    let atom = match residue["name"].as_str() {
        Some("ASN") => "ND2",
        Some("THR") => "OG1",
        _ => "OG",
    };
    let symbol = if anomer == "a" { "α" } else { "β" };
    format!(
        "{} ({symbol}1 → {atom})",
        components::residue_label(residue)
    )
}

fn root_anomer<'a>(ptm: &'a Value, sequence: &str) -> &'a str {
    if let Some(anomer) = ptm["root_anomer"].as_str().filter(|s| !s.is_empty()) {
        return anomer;
    }
    let compact: String = sequence.split_whitespace().collect();
    if let Some((_, linkage)) = compact.strip_suffix('-').and_then(|s| s.rsplit_once("(a"))
        && !linkage.is_empty()
        && linkage.bytes().all(|b| b.is_ascii_digit())
    {
        "a"
    } else {
        "b"
    }
}

impl Editor {
    fn residue(self, site: u64) -> Value {
        let mut residue = self.ui.part.with_untracked(|p| {
            p.as_ref()
                .and_then(|p| p.protein["residues"].as_array())
                .and_then(|r| r.iter().find(|r| r["id"] == site))
                .cloned()
                .unwrap_or_default()
        });
        if let Some(m) = self
            .ui
            .plan
            .get_untracked()
            .protein
            .mutations
            .iter()
            .find(|m| m["residue"] == residue["id"])
        {
            residue["name"] = m["target"].clone();
        }
        residue
    }
    pub(super) fn row(self, site: u64, ptm: Value, remove: impl Fn() + 'static) -> impl IntoView {
        let sequence: String = ptm["iupac"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("GlcNAc")
            .split_whitespace()
            .collect();
        let label = attachment_label(&self.residue(site), root_anomer(&ptm, &sequence));
        let remove_label = format!("Remove glycan from {label}");
        view! {
            <div class="protein-item glycan-item">
                <div>
                    <span>{label}</span>
                    <code>{sequence}</code>
                </div>
                <div class="glycan-buttons">
                    <button
                        type="button"
                        class="secondary compact"
                        on:click=move |_| self.open(site)
                    >
                        "Edit…"
                    </button>
                    <button
                        type="button"
                        class="remove-lipid"
                        aria-label=remove_label
                        on:click=move |_| remove()
                    >
                        "×"
                    </button>
                </div>
            </div>
        }
    }

    pub fn new(ui: Ui) -> Self {
        Self {
            ui,
            modal: Modal::new(),
            site: RwSignal::new(0),
            graph: RwSignal::new(Graph::default()),
            selected: RwSignal::new(0),
            history: RwSignal::new(vec![]),
            generation: StoredValue::new(0),
            text: RwSignal::new(String::new()),
            error: RwSignal::new(String::new()),
            pending: RwSignal::new(false),
            valid: RwSignal::new(false),
            tree: NodeRef::new(),
        }
    }
    pub(super) fn open(self, site: u64) {
        self.site.set(site);
        self.selected.set(0);
        self.graph.set(Graph::default());
        self.history.set(vec![]);
        let ptm = self.ui.plan.with_untracked(|p| {
            p.protein
                .ptms
                .iter()
                .find(|m| m["residue"] == site && m["type"] == "glycosylation")
                .cloned()
                .unwrap_or_default()
        });
        let value = ptm["iupac"].as_str().unwrap_or("GlcNAc");
        self.text.set(value.into());
        self.modal.open();
        self.request(
            json!({"iupac":value,"root_anomer": root_anomer(&ptm,value)}),
            true,
            false,
        );
    }
    fn request(self, mut body: Value, format: bool, remember: bool) {
        self.generation.update_value(|n| *n += 1);
        let generation = self.generation.get_value();
        let old = self.graph.get_untracked();
        body["site"] = self.residue(self.site.get_untracked())["name"].clone();
        if body.get("iupac").is_some() && body.get("root_anomer").is_none() {
            body["root_anomer"] = json!(old.root_anomer);
        }
        self.pending.set(true);
        self.valid.set(false);
        self.error.set(String::new());
        spawn_local(async move {
            if !format {
                gloo_timers::future::TimeoutFuture::new(300).await;
                if self.generation.get_value() != generation {
                    return;
                }
            }
            let response = api::post::<Graph>("/api/glycan", &body, None).await;
            if self.generation.get_value() != generation {
                return;
            }
            self.pending.set(false);
            match response {
                Ok(graph) => {
                    if remember && !old.nodes.is_empty() && old.nodes != graph.nodes {
                        self.history.update(|h| h.push(old.nodes));
                    }
                    if format {
                        self.text.set(graph.formatted_iupac.clone());
                    }
                    let next = graph
                        .id_map
                        .get(&self.selected.get_untracked())
                        .copied()
                        .or_else(|| graph.nodes.first().map(|n| n.id))
                        .unwrap_or(0);
                    self.selected.set(next);
                    self.graph.set(graph);
                    self.valid.set(true);
                }
                Err(e) => {
                    self.error.set(e);
                    self.valid
                        .set(body.get("nodes").is_some() && !old.nodes.is_empty());
                }
            }
        });
    }
    fn node(self) -> Option<Sugar> {
        let id = self.selected.get();
        self.graph
            .with(|g| g.nodes.iter().find(|n| n.id == id).cloned())
    }
    fn positions(self, parent: u64, except: Option<u64>) -> Vec<u8> {
        self.graph.with(|g| {
            g.acceptors
                .get(&parent)
                .into_iter()
                .flatten()
                .copied()
                .filter(|p| {
                    !g.nodes.iter().any(|n| {
                        n.parent == Some(parent) && Some(n.id) != except && n.position == Some(*p)
                    })
                })
                .collect()
        })
    }
    fn change(self, action: Change) {
        let mut nodes = self.graph.with_untracked(|g| g.nodes.clone());
        let id = self.selected.get_untracked();
        let Some(node) = nodes.iter_mut().find(|n| n.id == id) else {
            return;
        };
        match action {
            Change::Sugar(s) => node.sugar = s,
            Change::Anomer(s) => node.anomer = s,
            Change::Position(p) => node.position = Some(p),
            Change::Remove => {
                if node.parent.is_none() {
                    return;
                }
                let mut removed = BTreeSet::from([id]);
                loop {
                    let count = removed.len();
                    for n in &nodes {
                        if n.parent.is_some_and(|p| removed.contains(&p)) {
                            removed.insert(n.id);
                        }
                    }
                    if removed.len() == count {
                        break;
                    }
                }
                nodes.retain(|n| !removed.contains(&n.id));
            }
            Change::Add(sugar, anomer, position) => {
                let next = nodes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
                nodes.push(Sugar {
                    id: next,
                    sugar,
                    anomer,
                    position: Some(position),
                    parent: Some(id),
                });
                self.selected.set(next);
            }
        }
        self.request(json!({"nodes":nodes}), true, true);
    }
    fn pick_node(self, event: web_sys::Event) {
        if let Some(id) = event
            .target()
            .and_then(|n| n.dyn_into::<web_sys::Element>().ok())
            .and_then(|n| n.closest("[data-node-index]").ok().flatten())
            .and_then(|n| n.get_attribute("data-node-index"))
            .and_then(|v| v.parse().ok())
        {
            event.prevent_default();
            self.selected.set(id);
        }
    }
    fn render_tree(self) {
        let Some(host) = self.tree.get() else {
            return;
        };
        self.graph.with(|g| {
            host.set_text_content(None);
            let Some(svg) = web_sys::DomParser::new()
                .ok()
                .and_then(|p| {
                    p.parse_from_string(&g.svg, web_sys::SupportedType::ImageSvgXml)
                        .ok()
                })
                .and_then(|d| d.document_element())
                .and_then(|r| drawing(&r))
            else {
                return;
            };
            for (edge, text) in &g.linkage_labels {
                if let Ok(Some(label)) =
                    svg.query_selector(&format!("[data-edge-index='{edge}'] text.link"))
                {
                    label.set_text_content(Some(text));
                }
            }
            for node in &g.nodes {
                if let Ok(Some(shape)) =
                    svg.query_selector(&format!("[data-node-index='{}']", node.id))
                {
                    let pressed = node.id == self.selected.get();
                    let _ = shape.set_attribute("role", "button");
                    let _ = shape.set_attribute("tabindex", "0");
                    let _ = shape.set_attribute("aria-label", &node.sugar);
                    let _ =
                        shape.set_attribute("aria-pressed", if pressed { "true" } else { "false" });
                    let _ = shape
                        .class_list()
                        .toggle_with_force("glycan-selected", pressed);
                }
            }
            let _ = host.append_child(&svg);
        });
    }
}
fn drawing(source: &web_sys::Element) -> Option<web_sys::Element> {
    let tag = source.local_name();
    if ![
        "svg", "g", "rect", "circle", "line", "polygon", "polyline", "path", "ellipse", "text",
        "tspan", "title", "defs", "style",
    ]
    .contains(&tag.as_str())
        || tag == "style"
    {
        return None;
    }
    let e = document()
        .create_element_ns(Some("http://www.w3.org/2000/svg"), &tag)
        .ok()?;
    for attr in [
        "viewBox",
        "width",
        "height",
        "x",
        "y",
        "x1",
        "y1",
        "x2",
        "y2",
        "cx",
        "cy",
        "r",
        "rx",
        "ry",
        "d",
        "points",
        "transform",
        "class",
        "fill",
        "stroke",
        "stroke-width",
        "font-size",
        "text-anchor",
        "font-family",
        "data-node-index",
        "data-edge-index",
    ] {
        if let Some(v) = source.get_attribute(attr)
            && !v.contains("url(")
        {
            let _ = e.set_attribute(attr, &v);
        }
    }
    if matches!(tag.as_str(), "text" | "tspan" | "title") {
        e.set_text_content(source.text_content().as_deref());
    } else {
        let children = source.children();
        for i in 0..children.length() {
            if let Some(c) = children.item(i).and_then(|c| drawing(&c)) {
                let _ = e.append_child(&c);
            }
        }
    }
    Some(e)
}
#[component]
pub(super) fn GlycanPicker(editor: Editor) -> impl IntoView {
    let sugar = RwSignal::new("GlcNAc".to_owned());
    let anomer = RwSignal::new("b".to_owned());
    let preferred_position = RwSignal::new(None::<u8>);
    let invalid = move || !editor.valid.get();
    let root = move || editor.node().and_then(|n| n.parent).is_none();
    let sugars = move || editor.graph.with(|g| g.sugars.clone());
    let positions = move || editor.positions(editor.selected.get(), None);
    let position = move || {
        let rows = positions();
        preferred_position
            .get()
            .filter(|p| rows.contains(p))
            .or_else(|| rows.first().copied())
    };
    Effect::new(move |_| editor.render_tree());
    let save = move || {
        let site = editor.site.get_untracked();
        let graph = editor.graph.get_untracked();
        editor.ui.plan.update(|p| {
            if let Some(entry) = p
                .protein
                .ptms
                .iter_mut()
                .find(|p| p["residue"] == site && p["type"] == "glycosylation")
            {
                entry["iupac"] = json!(graph.iupac);
                entry["root_anomer"] = json!(graph.root_anomer);
            }
        });
        super::protein::changed(editor.ui);
        editor.modal.close();
    };
    view! {
        <dialog
            id="glycan-picker"
            aria-labelledby="glycan-heading"
            node_ref=editor.modal.0
            on:close=move |_| editor.generation.update_value(|n| *n += 1)
        >
            <h2 id="glycan-heading">"Edit glycan"</h2>
            <p id="glycan-attachment">
                {move || attachment_label(
                    &editor.residue(editor.site.get()),
                    &editor.graph.with(|g| g.root_anomer.clone()),
                )}
            </p>
            <div class="glycan-editor">
                <div class="glycan-text">
                    <label for="glycan-iupac">"IUPAC"</label>
                    <textarea
                        id="glycan-iupac"
                        rows="15"
                        spellcheck="false"
                        autocapitalize="off"
                        prop:value=move || editor.text.get()
                        on:input=move |e| {
                            let text = event_target_value(&e);
                            editor.text.set(text.clone());
                            editor.request(json!({"iupac":text}), false, true);
                        }
                    ></textarea>
                    <div class="actions">
                        <button
                            id="glycan-format"
                            class="secondary compact"
                            type="button"
                            on:click=move |_| {
                                editor
                                    .text
                                    .set(editor.graph.with_untracked(|g| g.formatted_iupac.clone()))
                            }
                            disabled=invalid
                        >
                            "Format"
                        </button>
                        <button
                            id="glycan-undo"
                            class="secondary compact"
                            type="button"
                            on:click=move |_| {
                                let old = if !editor.valid.get_untracked() {
                                    Some(editor.graph.with_untracked(|g| g.nodes.clone()))
                                } else {
                                    editor.history.try_update(Vec::pop).flatten()
                                };
                                if let Some(nodes) = old {
                                    editor.request(json!({"nodes":nodes}), true, false);
                                }
                            }
                            disabled=move || {
                                editor.pending.get()
                                    || (editor.history.with(Vec::is_empty)
                                        && (editor.valid.get()
                                            || editor.graph.with(|g| g.nodes.is_empty())))
                            }
                        >
                            "Undo"
                        </button>
                    </div>
                </div>
                <div class="glycan-graphic">
                    <div
                        id="glycan-tree"
                        aria-label="SNFG glycan tree"
                        node_ref=editor.tree
                        on:click=move |e| editor.pick_node(e.into())
                        on:keydown=move |e| {
                            if matches!(e.key().as_str(), "Enter" | " ") {
                                editor.pick_node(e.into());
                            }
                        }
                    ></div>
                    <fieldset id="glycan-tree-controls" disabled=invalid>
                        <h3>
                            "Selected sugar: "
                            <span id="glycan-selected-label">
                                {move || editor.node().map(|n| n.sugar)}
                            </span>
                        </h3>
                        <div class="glycan-fields">
                            <label>
                                "Sugar"
                                <select
                                    id="glycan-node-sugar"
                                    on:change=move |e| {
                                        editor.change(Change::Sugar(event_target_value(&e)))
                                    }
                                >
                                    {move || {
                                        sugars()
                                            .into_iter()
                                            .map(|v| {
                                                let selected = v
                                                    == (editor.node().map(|n| n.sugar).unwrap_or_default());
                                                view! {
                                                    <option value=v selected=selected>
                                                        {v.clone()}
                                                    </option>
                                                }
                                            })
                                            .collect_view()
                                    }}
                                </select>
                            </label>
                            <label>
                                "Anomer"
                                <select
                                    id="glycan-node-anomer"
                                    prop:value=move || {
                                        editor.node().map(|n| n.anomer).unwrap_or_default()
                                    }
                                    on:change=move |e| {
                                        editor.change(Change::Anomer(event_target_value(&e)))
                                    }
                                >
                                    <option value="a">"α"</option>
                                    <option value="b">"β"</option>
                                </select>
                            </label>
                            <label>
                                "Donor position"
                                <select
                                    id="glycan-node-donor"
                                    title="The donor is the sugar’s anomeric carbon."
                                    disabled
                                >
                                    {move || {
                                        editor
                                            .graph
                                            .with(|g| {
                                                g.donors
                                                    .get(&(editor.node().map(|n| n.sugar).unwrap_or_default()))
                                                    .copied()
                                            })
                                            .map(|d| view! { <option value=d>{d}</option> })
                                    }}
                                </select>
                            </label>
                            <label id="glycan-link-field" hidden=root>
                                "Acceptor position"
                                <select
                                    id="glycan-node-position"
                                    on:change=move |e| {
                                        if let Ok(p) = event_target_value(&e).parse() {
                                            editor.change(Change::Position(p));
                                        }
                                    }
                                >
                                    {move || {
                                        editor
                                            .node()
                                            .map(|n| {
                                                editor.positions(n.parent.unwrap_or(n.id), Some(n.id))
                                            })
                                            .unwrap_or_default()
                                            .into_iter()
                                            .map(|v| {
                                                let selected = v
                                                    == (editor.node().and_then(|n| n.position).unwrap_or(0));
                                                view! {
                                                    <option value=v selected=selected>
                                                        {v}
                                                    </option>
                                                }
                                            })
                                            .collect_view()
                                    }}
                                </select>
                            </label>
                            <button
                                id="glycan-remove"
                                class="secondary compact"
                                type="button"
                                disabled=root
                                on:click=move |_| editor.change(Change::Remove)
                            >
                                "Remove branch"
                            </button>
                        </div>
                        <h3>"Add to selected sugar"</h3>
                        <div class="glycan-fields">
                            <label>
                                "Sugar"
                                <select
                                    id="glycan-add-sugar"
                                    on:change=move |e| sugar.set(event_target_value(&e))
                                >
                                    {move || {
                                        sugars()
                                            .into_iter()
                                            .map(|v| {
                                                let selected = v == (sugar.get());
                                                view! {
                                                    <option value=v selected=selected>
                                                        {v.clone()}
                                                    </option>
                                                }
                                            })
                                            .collect_view()
                                    }}
                                </select>
                            </label>
                            <label>
                                "Anomer"
                                <select
                                    id="glycan-add-anomer"
                                    prop:value=move || anomer.get()
                                    on:change=move |e| anomer.set(event_target_value(&e))
                                >
                                    <option value="a">"α"</option>
                                    <option value="b" selected>
                                        "β"
                                    </option>
                                </select>
                            </label>
                            <label>
                                "Donor position"
                                <select
                                    id="glycan-add-donor"
                                    title="The donor is the sugar’s anomeric carbon."
                                    disabled
                                >
                                    {move || {
                                        editor
                                            .graph
                                            .with(|g| g.donors.get(&(sugar.get())).copied())
                                            .map(|d| view! { <option value=d>{d}</option> })
                                    }}
                                </select>
                            </label>
                            <label>
                                "Acceptor position"
                                <select
                                    id="glycan-add-position"
                                    on:change=move |e| {
                                        preferred_position.set(event_target_value(&e).parse().ok())
                                    }
                                >
                                    {move || {
                                        positions()
                                            .into_iter()
                                            .map(|v| {
                                                let selected = v == (position().unwrap_or(0));
                                                view! {
                                                    <option value=v selected=selected>
                                                        {v}
                                                    </option>
                                                }
                                            })
                                            .collect_view()
                                    }}
                                </select>
                            </label>
                            <button
                                id="glycan-add"
                                class="secondary compact"
                                type="button"
                                disabled=move || position().is_none()
                                on:click=move |_| {
                                    if let Some(p) = position() {
                                        editor
                                            .change(
                                                Change::Add(
                                                    sugar.get_untracked(),
                                                    anomer.get_untracked(),
                                                    p,
                                                ),
                                            );
                                    }
                                }
                            >
                                "Add sugar"
                            </button>
                        </div>
                    </fieldset>
                </div>
            </div>
            <p id="glycan-status" class="small" role="status" hidden=move || !editor.pending.get()>
                "Updating glycan…"
            </p>
            <p
                id="glycan-error"
                class="error"
                role="alert"
                hidden=move || editor.error.with(String::is_empty)
            >
                {move || editor.error.get()}
            </p>
            <div class="actions">
                <button id="glycan-apply" type="button" disabled=invalid on:click=move |_| save()>
                    "Apply"
                </button>
                <button
                    id="glycan-cancel"
                    class="secondary"
                    type="button"
                    on:click=move |_| editor.modal.close()
                >
                    "Cancel"
                </button>
            </div>
        </dialog>
    }
}
