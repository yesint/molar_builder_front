use super::*;

#[derive(Clone)]
pub(super) struct Download {
    name: String,
    storage_key: String,
    layers: Vec<crate::viewer::Layer>,
    pub(super) geometry: Value,
    pub(super) prepared: Option<std::sync::Arc<Vec<u8>>>,
    pub(super) key: String,
    elapsed: f64,
    archive: std::sync::Arc<Vec<u8>>,
    coordinates: std::sync::Arc<Vec<u8>>,
    ligands: Vec<std::sync::Arc<Vec<u8>>>,
}

impl Download {
    pub(super) fn has_ligand(&self, index: usize) -> bool {
        index < self.ligands.len()
    }
    pub(super) fn ligand_bytes(&self, index: usize) -> Option<std::sync::Arc<Vec<u8>>> {
        self.ligands.get(index).cloned()
    }

    pub(super) fn preview(&self, viewer: Viewer) {
        viewer.send(Action::Show(Scene {
            path: String::new(),
            name: "system.cif".into(),
            selection: "all".into(),
            layers: self.layers.clone(),
            ligand: None,
            bytes: Some(self.coordinates.clone()),
        }));
    }
}

fn key_for(plan: &Plan, mode: &str) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let s = window()
        .session_storage()
        .map_err(|_| "Session storage unavailable")?
        .ok_or("Session storage unavailable")?;
    let hash = Sha256::digest(serde_json::to_vec(plan).map_err(|e| e.to_string())?);
    let name = format!("molar-rust-{mode}-{hash:x}");
    if let Some(key) = s.get_item(&name).map_err(|_| "Cannot read checkout")? {
        return Ok(key);
    }
    let mut bytes = [0; 32];
    window()
        .crypto()
        .map_err(|_| "Browser crypto unavailable")?
        .get_random_values_with_u8_array(&mut bytes)
        .map_err(|_| "Cannot create job key")?;
    let key = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    s.set_item(&name, &key)
        .map_err(|_| "Cannot save checkout")?;
    Ok(key)
}
pub(super) async fn save_file(name: &str, bytes: &[u8]) -> Result<(), String> {
    let array = js_sys::Uint8Array::from(bytes);
    let chunks = js_sys::Array::new();
    chunks.push(&array);
    let blob =
        web_sys::Blob::new_with_u8_array_sequence(&chunks).map_err(|_| "Cannot create download")?;
    let link: web_sys::HtmlAnchorElement = document()
        .create_element("a")
        .map_err(|_| "Cannot create download")?
        .unchecked_into();
    let url =
        web_sys::Url::create_object_url_with_blob(&blob).map_err(|_| "Cannot create download")?;
    link.set_href(&url);
    link.set_download(name);
    link.click();
    gloo_timers::future::TimeoutFuture::new(500).await;
    web_sys::Url::revoke_object_url(&url).map_err(|_| "Cannot release download URL".to_string())
}

async fn run_submission(
    ui: Ui,
    health: &Value,
    popup: Option<&web_sys::Window>,
) -> Result<(), String> {
    let started = js_sys::Date::now();
    ui.viewer.save().await?;
    let plan = ui.plan.get_untracked();
    plan.validate(&ui.catalog.get_untracked())?;
    let mode = health["mode"].as_str().ok_or("Server unavailable")?;
    let k = key_for(&plan, mode)?;
    let order: Value = api::post("/api/checkout", &plan, Some(&k)).await?;
    if let Some(p) = popup {
        if order["amount"] != health["amount"] {
            return Err("Checkout price changed. Reload the builder to review the price.".into());
        }
        let url = order["url"].as_str().ok_or("No approval URL")?;
        let target = web_sys::Url::new(url).map_err(|_| "Invalid PayPal URL")?;
        if target.origin()
            != if mode == "sandbox" {
                "https://www.sandbox.paypal.com"
            } else {
                "https://www.paypal.com"
            }
        {
            return Err("Unexpected PayPal approval URL".into());
        }
        let order_id = order["order_id"]
            .as_str()
            .ok_or("Missing PayPal order ID")?;
        let cancellation_key = format!("molar-cancelled-{order_id}");
        if let Ok(Some(storage)) = window().local_storage() {
            let _ = storage.remove_item(&cancellation_key);
        }
        if let Ok(Some(storage)) = p.session_storage() {
            let _ = storage.set_item("molar-popup-order", order_id);
        }
        p.location()
            .replace(url)
            .map_err(|_| "Cannot open checkout")?;
        ui.logs.set(vec![(
            js_sys::Date::now(),
            "Waiting for PayPal approval and signed confirmation.".into(),
        )]);
        let start = js_sys::Date::now();
        loop {
            let v: Value = api::post("/api/payment-status", &json!({}), Some(&k)).await?;
            if v["paid"] == true {
                break;
            }
            // The return tab records the outcome under this order, including when
            // PayPal isolates its popup from the original window.
            let cancelled = window()
                .local_storage()
                .ok()
                .flatten()
                .and_then(|storage| storage.get_item(&cancellation_key).ok().flatten())
                .is_some();
            if cancelled {
                return Err("You cancelled checkout. Submit again to resume.".into());
            }
            if p.closed().unwrap_or(false) || js_sys::Date::now() - start > 1_200_000. {
                return Err(
                    "Checkout closed or expired. Submit the same settings to resume.".into(),
                );
            }
            gloo_timers::future::TimeoutFuture::new(2500).await;
        }
        let _ = p.close();
    }
    ui.logs
        .set(vec![(js_sys::Date::now(), "Starting build…".into())]);
    let done = std::rc::Rc::new(std::cell::Cell::new(false));
    let stop = done.clone();
    let key = k.clone();
    spawn_local(async move {
        while !stop.get() {
            if let Some(entries) = progress(&key).await
                && !stop.get()
            {
                ui.logs.set(entries);
            }
            gloo_timers::future::TimeoutFuture::new(1200).await;
        }
    });
    let r = api::post::<Value>("/api/build", &json!({}), Some(&k)).await;
    done.set(true);
    if let Some(entries) = progress(&k).await {
        ui.logs.set(entries);
    }
    r?;
    ui.logs
        .update(|v| v.push((js_sys::Date::now(), "Load final system".into())));
    let (archive, coordinates) = futures_util::try_join!(
        api::bytes("/api/result/assembly.zip", Some(&k)),
        api::bytes("/api/result/system.cif", Some(&k))
    )?;
    let count = ui
        .part
        .with_untracked(|part| part.as_ref().map_or(0, |p| p.ligands.len()));
    let mut ligands = Vec::with_capacity(count);
    for index in 0..count {
        let path = format!("/api/result/ligand-{index}.zip");
        ligands.push(api::bytes(&path, Some(&k)).await?.into());
    }
    let metadata: Value =
        serde_json::from_slice(&api::bytes("/api/result/viewer.json", Some(&k)).await?)
            .map_err(|e| e.to_string())?;
    let layers = assembly_layers(ui, &metadata);
    let prepared = if metadata["prepared"].is_object() {
        Some(
            api::bytes("/api/result/prepared.cif", Some(&k))
                .await?
                .into(),
        )
    } else {
        None
    };
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(serde_json::to_vec(&plan).map_err(|e| e.to_string())?);
    let result = Download {
        storage_key: format!("molar-rust-{mode}-{hash:x}"),
        key: k.clone(),
        elapsed: js_sys::Date::now() - started,
        geometry: metadata,
        prepared,
        layers,
        name: "assembly.zip".into(),
        archive: archive.into(),
        ligands,
        coordinates: coordinates.into(),
    };
    ui.set_done(5, true);
    if ui.stage.get_untracked() == 5 {
        visible("assembly-viewer", true);
        result.preview(ui.viewer);
    }
    ui.result.set(Some(result));
    Ok(())
}
fn assembly_layers(ui: Ui, metadata: &Value) -> Vec<crate::viewer::Layer> {
    use crate::viewer::{Layer, PALETTE};
    let mut out = vec![];
    if let Some(m) = ui.plan.get_untracked().membrane {
        for (i, row) in m
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r.lower + r.upper > 0.)
        {
            let name = row.id.chars().take(5).collect::<String>();
            out.push(Layer::new(
                "lipid",
                format!("resname {name}"),
                "licorice",
                PALETTE[i % PALETTE.len()],
            ));
        }
    }
    if let Some(sel) = metadata["protein_selection"]
        .as_str()
        .filter(|s| !s.is_empty() && *s != "not all")
    {
        out.push(Layer::new("protein", sel.into(), "cartoon", "chain"));
        for mut detail in protein::details(ui, true) {
            detail.selection = format!("({sel}) and ({})", detail.selection);
            out.push(detail);
        }
    }
    if let Some(sel) = metadata["ligand_selection"].as_str() {
        out.push(Layer::new("ligands", sel.into(), "ballstick", "element"));
    }
    out.push(Layer::new(
        "water",
        "resname SOL".into(),
        "lines",
        "element",
    ));
    out.push(Layer::new(
        "ions",
        "resname SOD POT CAL MG CLA".into(),
        "vdw",
        "element",
    ));
    out
}
pub(super) fn setup(ui: Ui) {
    let returned = window().location().search().unwrap_or_default();
    let parameter = |name: &str| {
        returned
            .trim_start_matches('?')
            .split('&')
            .filter_map(|s| s.split_once('='))
            .find(|(key, _)| *key == name)
            .map(|(_, v)| v.to_owned())
    };
    let outcome = parameter("checkout");
    if outcome.as_deref() == Some("cancel") {
        let stored = window()
            .session_storage()
            .ok()
            .flatten()
            .and_then(|s| s.get_item("molar-popup-order").ok().flatten());
        if let Some(order) = parameter("token").or(stored)
            && (8..=32).contains(&order.len())
            && order
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
            && let Ok(Some(storage)) = window().local_storage()
        {
            let _ = storage.set_item(&format!("molar-cancelled-{order}"), "true");
        }
    }
    if matches!(outcome.as_deref(), Some("return" | "cancel")) {
        notice(
            "checkout-return",
            if returned.contains("cancel") {
                "You cancelled checkout. You can resume it in the builder tab where you submitted the job."
            } else {
                "You returned from PayPal. The builder tab where you submitted the job shows its progress."
            },
        );
    }
    let terms = el("public-beta-terms");
    let fee = el("submission-fee");
    let children = terms.children();
    for i in 0..children.length() {
        fee.append_child(
            &children
                .item(i)
                .unwrap()
                .clone_node_with_deep(true)
                .unwrap(),
        )
        .unwrap();
    }
    el("force-field-download")
        .set_attribute("href", &api::url("/api/assembly/force-field.zip"))
        .unwrap();
    mount("submit-summary", move || {
        move || {
            let p = ui.plan.get();
            let flags = ui.confirmed.get();
            let mut steps = vec![];
            if ui.has_protein() && flags[1] {
                let mut details = vec![];
                if p.protein.orient && p.membrane.is_some() {
                    details.push("orientation".to_string());
                }
                for m in p.protein.mutations {
                    details.push(format!(
                        "resindex {} → {}",
                        m["residue"],
                        m["target"].as_str().unwrap_or("")
                    ));
                }
                for m in p.protein.ptms {
                    let name = if m["iupac"].as_str().is_some_and(|s| !s.is_empty()) {
                        "glycan".to_owned()
                    } else {
                        m["type"].as_str().unwrap_or("").replace('_', " ")
                    };
                    if !details.contains(&name) {
                        details.push(name);
                    }
                }
                if !p.protein.ss_bonds.is_empty() {
                    details.push("disulfide bonds".into());
                }
                steps.push(with_details("Prepare protein", &details));
            }
            if flags[2]
                && let Some(part) = ui.part.get()
                && !part.ligands.is_empty()
            {
                let mut counts = indexmap::IndexMap::new();
                for (name, _, _) in ligands::descriptions(ui, &part) {
                    *counts.entry(name).or_insert(0usize) += 1;
                }
                let names = counts
                    .into_iter()
                    .map(|(name, count)| {
                        if count == 1 {
                            name
                        } else {
                            format!("{name} ×{count}")
                        }
                    })
                    .collect::<Vec<_>>();
                steps.push(with_details("Parameterize ligands", &names));
            }
            if (flags[1] && ui.has_protein()) || (flags[2] && ui.has_ligands()) {
                steps.push("Combine components".into());
            }
            if let Some(m) = p.membrane
                && flags[3]
            {
                steps.push(with_details(
                    "Build membrane",
                    &m.rows.into_iter().map(|r| r.id).collect::<Vec<_>>(),
                ));
            }
            if let Some(s) = p.solvent {
                let mut details = vec![format!("{} mol/L {}", s.concentration, s.salt)];
                if s.neutralize {
                    details.push("neutralize".into());
                }
                steps.push(with_details("Add solvent and ions", &details));
            }
            steps.extend([
                "Minimize assembled system".into(),
                "Load final system".into(),
            ]);
            steps
                .into_iter()
                .map(|s| view! { <li>{s}</li> })
                .collect_view()
        }
    });
    let popup = StoredValue::new_local(None::<web_sys::Window>);
    let waiting = RwSignal::new(false);
    click("assemble-system", move || {
        let h = ui.health.get_untracked();
        if !ui.ready() || !ui.confirmed.get_untracked()[4] {
            return;
        }
        let window = if h["mode"] == "local" {
            None
        } else {
            match window().open_with_url_and_target("about:blank", "_blank") {
                Ok(Some(w)) => Some(w),
                _ => {
                    ui.msg
                        .set("Allow the PayPal checkout window and submit again.".into());
                    return;
                }
            }
        };
        popup.set_value(window.clone());
        waiting.set(window.is_some());
        ui.logs.set(vec![]);
        ui.task(async move {
            let result = run_submission(ui, &h, window.as_ref()).await;
            if let Some(w) = window {
                let _ = w.close();
            }
            popup.set_value(None);
            waiting.set(false);
            result
        });
    });
    click("cancel-checkout", move || {
        if let Some(w) = popup.get_value() {
            let _ = w.close();
        }
        waiting.set(false);
    });
    let unsaved = RwSignal::new(false);
    Effect::new(move |_| {
        if ui.result.get().is_some() {
            unsaved.set(true);
        }
    });
    listen(window().as_ref(), "beforeunload", move |event| {
        if unsaved.get_untracked() || ui.busy.get_untracked() {
            event.prevent_default();
        }
    });
    click("assembly-download", move || {
        if let Some(r) = ui.result.get_untracked() {
            spawn_local(async move {
                match save_file(&r.name, &r.archive).await {
                    Ok(()) => {
                        unsaved.set(false);
                        match api::post::<Value>("/api/received", &json!({}), Some(&r.key)).await {
                            Ok(_) => {
                                if let Ok(Some(storage)) = window().session_storage() {
                                    let _ = storage.remove_item(&r.storage_key);
                                }
                            }
                            Err(error) => notice(
                                "archive-notice",
                                &format!(
                                    "The ZIP was saved. The server could not release its copy: {error}"
                                ),
                            ),
                        }
                    }
                    Err(e) => ui.msg.set(e),
                }
            });
        }
    });
    click("retry-viewer", move || {
        if let Some(r) = ui.result.get_untracked() {
            r.preview(ui.viewer);
        }
    });
    mount("assembly-log", move || {
        move || {
            let entries = ui.logs.get();
            let finished = ui.confirmed.get()[5];
            let error = !ui.msg.get().is_empty() && !ui.busy.get();
            entries
                .iter()
                .enumerate()
                .map(|(i, (started, name))| {
                    let end = entries.get(i + 1).map(|r| r.0);
                    let (state, marker) = if end.is_some() || finished {
                        ("done", "✓")
                    } else if error {
                        ("failed", "×")
                    } else {
                        ("running", "…")
                    };
                    let duration = end.map(|next| elapsed(next - started)).unwrap_or_default();
                    view! {
                        <li data-state=state>
                            <div class="log-line">
                                <span class="stage-mark">{marker}</span>
                                <span>{name.clone()}</span>
                                <time>{duration}</time>
                            </div>
                        </li>
                    }
                })
                .collect_view()
        }
    });
    Effect::new(move |_| {
        let ready = ui.ready();
        let flags = ui.confirmed.get();
        let busy = ui.busy.get();
        let result = ui.result.get();
        let done = flags[5];
        let h = ui.health.get();
        let mode = h["mode"].as_str();
        enabled("assemble-system", ready && flags[4] && !busy);
        visible("assemble-system", !done);
        if matches!(mode, Some("sandbox" | "production")) {
            let price = format!("USD {}", h["amount"]["value"].as_str().unwrap_or("0.50"));
            label(
                "assemble-system",
                &format!("Submit and pay {price} with PayPal"),
            );
            for id in ["public-beta-terms", "submission-fee"] {
                if let Ok(Some(node)) = el(id).query_selector("[data-build-price]") {
                    node.set_text_content(Some(&price));
                }
            }
        }
        visible("assembly-progress", busy && ui.stage.get() == 5);
        visible("assembly-log", !ui.logs.with(Vec::is_empty));
        visible(
            "cancel-checkout",
            waiting.get()
                && ui.logs.with(|v| {
                    v.last()
                        .is_some_and(|r| r.1.starts_with("Waiting for PayPal"))
                }),
        );
        visible("assembly-unsaved-warning", unsaved.get());
        visible("assembly-download", result.is_some());
        label(
            "assembly-download",
            if done {
                "Save ZIP"
            } else {
                "Save previous ZIP"
            },
        );
        visible("assembly-viewer", done);
        visible("build-total", done);
        if let Some(r) = &result {
            label("build-duration", &elapsed(r.elapsed));
        }
        if let Some((_, last)) = ui.logs.with(|v| v.last().cloned()) {
            label("assembly-status", &last);
        }

        visible("retry-viewer", done && !ui.msg.get().is_empty());
    });
}
fn with_details(label: &str, details: &[String]) -> String {
    if details.is_empty() {
        label.into()
    } else {
        format!("{label} ({})", details.join(", "))
    }
}

async fn progress(key: &str) -> Option<Vec<(f64, String)>> {
    let bytes = api::bytes("/api/result/progress", Some(key)).await.ok()?;
    let entries: Vec<(f64, String)> = serde_json::from_slice(&bytes).ok()?;
    (!entries.is_empty()).then_some(entries)
}
fn elapsed(milliseconds: f64) -> String {
    let seconds = milliseconds.max(0.) / 1000.;
    if seconds < 60. {
        format!("{seconds:.2} s")
    } else {
        format!("{} min {} s", seconds as u64 / 60, seconds as u64 % 60)
    }
}
