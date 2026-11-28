use super::*;
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
#[derive(Clone, Copy)]
pub(super) struct Editor {
    ui: Ui,
    site: RwSignal<u64>,
    graph: RwSignal<Value>,
    selected: RwSignal<u64>,
    history: RwSignal<Vec<Value>>,
    generation: StoredValue<u64>,
}
fn textarea() -> web_sys::HtmlTextAreaElement {
    el("glycan-iupac").unchecked_into()
}
fn strings(v: &Value) -> Vec<(String, String)> {
    v.as_array()
        .into_iter()
        .flatten()
        .map(|x| {
            let s = x
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| x.to_string());
            (s.clone(), s)
        })
        .collect()
}
impl Editor {
    fn residue(self) -> Value {
        let mut residue = self.ui.part.with_untracked(|p| {
            p.as_ref()
                .and_then(|p| p.protein["residues"].as_array())
                .and_then(|r| r.iter().find(|r| r["id"] == self.site.get_untracked()))
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
    pub(super) fn open(self, site: u64) {
        self.site.set(site);
        self.selected.set(0);
        self.graph.set(Value::Null);
        self.history.set(vec![]);
        label("glycan-tree", "");
        let r = self.residue();
        label("glycan-attachment", &components::residue_label(&r));
        let previous = self
            .ui
            .plan
            .get_untracked()
            .protein
            .ptms
            .into_iter()
            .find(|m| m["residue"] == site && m["type"] == "glycosylation");
        let value = previous
            .as_ref()
            .and_then(|p| p["iupac"].as_str())
            .unwrap_or("GlcNAc");
        textarea().set_value(value);
        let _ = dialog("glycan-picker").show_modal();
        self.request(json!({"iupac":value,"root_anomer":previous.as_ref().and_then(|p|p["root_anomer"].as_str()).unwrap_or("b")}),true,false);
    }
    fn request(self, mut body: Value, format: bool, remember: bool) {
        self.generation.update_value(|n| *n += 1);
        let gen_id = self.generation.get_value();
        let old = self.graph.get_untracked();
        body["site"] = self.residue()["name"].clone();
        if body.get("iupac").is_some() && body.get("root_anomer").is_none() {
            body["root_anomer"] = json!(old["root_anomer"].as_str().unwrap_or("b"));
        }
        enabled("glycan-apply", false);
        enabled("glycan-tree-controls", false);
        visible("glycan-status", true);
        visible("glycan-error", false);
        spawn_local(async move {
            if !format {
                gloo_timers::future::TimeoutFuture::new(300).await;
                if self.generation.get_value() != gen_id {
                    return;
                }
            }
            let result = api::post::<Value>("/api/glycan", &body, None).await;
            if self.generation.get_value() != gen_id {
                return;
            }
            visible("glycan-status", false);
            match result {
                Ok(value) => {
                    if remember && !old.is_null() && old["nodes"] != value["nodes"] {
                        self.history.update(|v| v.push(old));
                    }
                    if format {
                        textarea().set_value(value["formatted_iupac"].as_str().unwrap_or_default());
                    }
                    if let Some(id) =
                        value["id_map"][self.selected.get_untracked().to_string()].as_u64()
                    {
                        self.selected.set(id);
                    }
                    self.graph.set(value);
                    self.render();
                }
                Err(err) => {
                    notice("glycan-error", &err);
                    if body.get("nodes").is_some() && !old.is_null() {
                        self.render();
                    } else {
                        enabled("glycan-format", false);
                    }
                    enabled("glycan-undo", !old.is_null());
                }
            }
        });
    }
    fn render(self) {
        let graph = self.graph.get_untracked();
        let svg = graph["svg"].as_str().unwrap_or("");
        if let Ok(parser) = web_sys::DomParser::new()
            && let Ok(doc) = parser.parse_from_string(svg, web_sys::SupportedType::ImageSvgXml)
            && let Some(root) = doc.document_element()
        {
            // Copy only drawing primitives and presentation attributes from the generated SNFG.
            let host = el("glycan-tree");
            host.set_text_content(None);
            if let Some(safe) = drawing(&root) {
                host.append_child(&safe).unwrap();
            }
        }
        if let Some(labels) = graph["linkage_labels"].as_object() {
            for (edge, text) in labels {
                if let Ok(Some(label)) = el("glycan-tree")
                    .query_selector(&format!("[data-edge-index='{edge}'] text.link"))
                {
                    label.set_text_content(text.as_str());
                }
            }
        }
        let r = self.residue();
        label(
            "glycan-attachment",
            &format!(
                "{} ({}1 → {})",
                components::residue_label(&r),
                if graph["root_anomer"] == "a" {
                    "α"
                } else {
                    "β"
                },
                match r["name"].as_str() {
                    Some("ASN") => "ND2",
                    Some("THR") => "OG1",
                    _ => "OG",
                }
            ),
        );
        options("glycan-node-sugar", strings(&graph["sugars"]));
        let first = select("glycan-add-sugar").value().is_empty();
        options("glycan-add-sugar", strings(&graph["sugars"]));
        if first {
            select("glycan-add-sugar").set_value("GlcNAc");
        }
        enabled("glycan-tree-controls", true);
        enabled("glycan-apply", true);
        enabled("glycan-format", true);
        enabled("glycan-undo", !self.history.with_untracked(Vec::is_empty));
        self.controls();
    }
    fn controls(self) {
        let g = self.graph.get_untracked();
        let Some(nodes) = g["nodes"].as_array() else {
            return;
        };
        let Some(node) = nodes
            .iter()
            .find(|n| n["id"] == self.selected.get_untracked())
            .or_else(|| nodes.iter().find(|n| n["parent"].is_null()))
        else {
            return;
        };
        let id = node["id"].as_u64().unwrap();
        self.selected.set(id);
        for n in nodes {
            let key = n["id"].as_u64().unwrap();
            if let Ok(Some(shape)) =
                el("glycan-tree").query_selector(&format!("[data-node-index='{key}']"))
            {
                shape.set_attribute("role", "button").unwrap();
                shape.set_attribute("tabindex", "0").unwrap();
                shape
                    .set_attribute("aria-label", n["sugar"].as_str().unwrap_or(""))
                    .unwrap();
                shape
                    .set_attribute("aria-pressed", if key == id { "true" } else { "false" })
                    .unwrap();
                let _ = shape
                    .class_list()
                    .toggle_with_force("glycan-selected", key == id);
            }
        }
        let root = node["parent"].is_null();
        let sugar = node["sugar"].as_str().unwrap_or("");
        label("glycan-selected-label", sugar);
        select("glycan-node-sugar").set_value(sugar);
        select("glycan-node-anomer").set_value(node["anomer"].as_str().unwrap_or("b"));
        visible("glycan-link-field", !root);
        enabled("glycan-remove", !root);
        for (prefix, parent, except) in [
            ("node", node["parent"].as_u64().unwrap_or(id), Some(id)),
            ("add", id, None),
        ] {
            let k = format!("glycan-{prefix}-");
            let sugar = select(&(k.clone() + "sugar")).value();
            let donor = g["donors"][&sugar].to_string();
            options(&(k.clone() + "donor"), [(donor.clone(), donor)]);
            enabled(&(k.clone() + "donor"), false);
            let positions = g["acceptors"][parent.to_string()]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|v| {
                    !nodes.iter().any(|n| {
                        n["parent"] == parent && n["id"].as_u64() != except && n["position"] == **v
                    })
                })
                .map(|v| (v.to_string(), v.to_string()))
                .collect::<Vec<_>>();
            if prefix == "add" {
                enabled("glycan-add", !positions.is_empty());
            }
            options(&(k + "position"), positions);
        }
        if !root {
            select("glycan-node-position").set_value(&node["position"].to_string());
        }
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
pub(super) fn setup(ui: Ui) -> Editor {
    let editor = Editor {
        ui,
        site: RwSignal::new(0),
        graph: RwSignal::new(Value::Null),
        selected: RwSignal::new(0),
        history: RwSignal::new(vec![]),
        generation: StoredValue::new(0),
    };
    on("glycan-iupac", "input", move |_| {
        editor.request(json!({"iupac":textarea().value()}), false, true)
    });
    click("glycan-format", move || {
        textarea().set_value(
            editor.graph.get_untracked()["formatted_iupac"]
                .as_str()
                .unwrap_or(""),
        )
    });
    for event in ["click", "keydown"] {
        on("glycan-tree", event, move |e| {
            if event == "keydown"
                && e.dyn_ref::<web_sys::KeyboardEvent>()
                    .is_none_or(|e| !matches!(e.key().as_str(), "Enter" | " "))
            {
                return;
            }
            if let Some(node) = e
                .target()
                .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                .and_then(|t| t.closest("[data-node-index]").ok().flatten())
                .and_then(|t| t.get_attribute("data-node-index"))
                .and_then(|s| s.parse().ok())
            {
                e.prevent_default();
                editor.selected.set(node);
                editor.controls();
            }
        });
    }
    on("glycan-add-sugar", "change", move |_| editor.controls());
    for control in [
        "glycan-node-sugar",
        "glycan-node-anomer",
        "glycan-node-position",
        "glycan-add",
        "glycan-remove",
    ] {
        on(
            control,
            if matches!(control, "glycan-add" | "glycan-remove") {
                "click"
            } else {
                "change"
            },
            move |_| {
                let g = editor.graph.get_untracked();
                let Some(mut nodes) = g["nodes"].as_array().cloned() else {
                    return;
                };
                let id = editor.selected.get_untracked();
                match control {
                    "glycan-remove" => remove_branch(&mut nodes, id),
                    "glycan-add" => {
                        let next = nodes
                            .iter()
                            .filter_map(|n| n["id"].as_u64())
                            .max()
                            .unwrap_or(0)
                            + 1;
                        nodes.push(json!({"id":next,"parent":id,"sugar":select("glycan-add-sugar").value(),"anomer":select("glycan-add-anomer").value(),"position":select("glycan-add-position").value().parse::<u8>().ok()}));
                        editor.selected.set(next);
                    }
                    _ => {
                        if let Some(n) = nodes.iter_mut().find(|n| n["id"] == id) {
                            n["sugar"] = json!(select("glycan-node-sugar").value());
                            n["anomer"] = json!(select("glycan-node-anomer").value());
                            if !n["parent"].is_null() {
                                n["position"] = json!(
                                    select("glycan-node-position").value().parse::<u8>().ok()
                                );
                            }
                        }
                    }
                }
                editor.request(json!({"nodes":nodes}), true, true);
            },
        );
    }
    click("glycan-undo", move || {
        let old = if el("glycan-apply").has_attribute("disabled") {
            Some(editor.graph.get_untracked())
        } else {
            editor.history.try_update(Vec::pop).flatten()
        };
        if let Some(old) = old {
            editor.request(json!({"nodes":old["nodes"]}), true, false);
        }
    });
    click("glycan-cancel", || dialog("glycan-picker").close());
    on("glycan-picker", "close", move |_| {
        editor.generation.update_value(|n| *n += 1)
    });
    click("glycan-apply", move || {
        let g = editor.graph.get_untracked();
        let site = editor.site.get_untracked();
        ui.plan.update(|p|{p.protein.ptms.retain(|m|m["residue"]!=site);p.protein.ptms.push(json!({"type":"glycosylation","residue":site,"iupac":g["iupac"],"root_anomer":g["root_anomer"]}));});
        ui.set_done(1, false);
        ui.set_done(5, false);
        dialog("glycan-picker").close();
    });
    editor
}
