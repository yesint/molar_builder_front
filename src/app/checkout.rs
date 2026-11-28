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
    ui.set_done(Page::Submit, true);
    if ui.stage.get_untracked() == Page::Submit {
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
pub(super) fn checkout_return() -> Option<&'static str> {
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
    match outcome.as_deref() {
        Some("cancel") => Some(
            "You cancelled checkout. You can resume it in the builder tab where you submitted the job.",
        ),
        Some("return") => Some(
            "You returned from PayPal. The builder tab where you submitted the job shows its progress.",
        ),
        _ => None,
    }
}
fn summary(ui: Ui) -> impl IntoView {
    move || {
        let p = ui.plan.get();
        let flags = ui.confirmed.get();
        let mut steps = vec![];
        if ui.has_protein() && flags[Page::Protein as usize] {
            let mut details = vec![];
            if p.protein.orient && p.membrane.is_some() {
                details.push("orientation".to_string());
            }
            for m in p.protein.mutations {
                details.push(format!("resindex {} → {}", m.residue, m.target));
            }
            for m in p.protein.ptms {
                let name = match m.modification {
                    Modification::Glycosylation(_) => "glycan".into(),
                    other => other.key().replace('_', " "),
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
        if flags[Page::Ligands as usize]
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
        if (flags[Page::Protein as usize] && ui.has_protein())
            || (flags[Page::Ligands as usize] && ui.has_ligands())
        {
            steps.push("Combine components".into());
        }
        if let Some(m) = p.membrane
            && flags[Page::Membrane as usize]
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
}
fn build_log(ui: Ui) -> impl IntoView {
    move || {
        let entries = ui.logs.get();
        let finished = ui.confirmed.get()[Page::Submit as usize];
        let error = !ui.error(Page::Submit).get().is_empty() && !ui.busy.get();
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
}
#[component]
pub(super) fn SubmitPage(ui: Ui) -> impl IntoView {
    let archive_notice = RwSignal::new(String::new());
    let popup = StoredValue::new_local(None::<web_sys::Window>);
    let waiting = RwSignal::new(false);
    let submit = move |_| {
        let h = ui.health.get_untracked();
        if !ui.ready() || !ui.confirmed.get_untracked()[Page::Solvent as usize] {
            return;
        }
        let window = if h["mode"] == "local" {
            None
        } else {
            match window().open_with_url_and_target("about:blank", "_blank") {
                Ok(Some(w)) => Some(w),
                _ => {
                    ui.error(Page::Submit)
                        .set("Allow the PayPal checkout window and submit again.".into());
                    return;
                }
            }
        };
        archive_notice.set(String::new());
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
    };
    let cancel = move |_| {
        if let Some(w) = popup.get_value() {
            let _ = w.close();
        }
        waiting.set(false);
    };
    let unsaved = RwSignal::new(false);
    Effect::new(move |_| {
        if ui.result.get().is_some() {
            unsaved.set(true);
        }
    });
    let unload = window_event_listener(leptos::ev::beforeunload, move |event| {
        if unsaved.get_untracked() || ui.busy.get_untracked() {
            event.prevent_default();
        }
    });
    on_cleanup(move || unload.remove());
    let save = move |_| {
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
                            Err(error) => archive_notice.set(format!(
                                "The ZIP was saved. The server could not release its copy: {error}"
                            )),
                        }
                    }
                    Err(e) => ui.error(Page::Submit).set(e),
                }
            });
        }
    };
    let retry = move |_| {
        if let Some(r) = ui.result.get_untracked() {
            r.preview(ui.viewer);
        }
    };
    let done = move || ui.confirmed.get()[Page::Submit as usize];
    view! {
        <StageHeading ui page=Page::Submit />
        <ol id="submit-summary" aria-label="Planned build steps">
            {summary(ui)}
        </ol>
        <div id="submission-fee" class="notice">
            <BetaTerms ui />
        </div>
        <div class="actions">
            <button
                id="assemble-system"
                type="button"
                disabled=move || {
                    !ui.ready() || !ui.confirmed.get()[Page::Solvent as usize] || ui.busy.get()
                }
                hidden=done
                on:click=submit
            >
                {move || {
                    if ui.paid() {
                        format!("Submit and pay {} with PayPal", ui.price())
                    } else {
                        "Submit".into()
                    }
                }}
            </button>
            <button
                id="cancel-checkout"
                class="secondary"
                type="button"
                hidden=move || {
                    !(waiting.get()
                        && ui
                            .logs
                            .with(|v| {
                                v.last().is_some_and(|r| r.1.starts_with("Waiting for PayPal"))
                            }))
                }
                on:click=cancel
            >
                "Cancel checkout"
            </button>
            <p
                id="assembly-unsaved-warning"
                class="notice"
                role="status"
                hidden=move || !unsaved.get()
            >
                "If you reload or close this tab before saving the ZIP, you will lose the result."
            </p>
            <button
                id="assembly-download"
                type="button"
                aria-describedby="assembly-unsaved-warning"
                hidden=move || ui.result.with(Option::is_none)
                on:click=save
            >
                {move || if done() { "Save ZIP" } else { "Save previous ZIP" }}
            </button>
            <a id="assembly-archive" download="assembly.zip" hidden></a>
            <a id="force-field-download" href=api::url("/api/assembly/force-field.zip")>
                "Download CHARMM36 / CGenFF force field"
            </a>
            <button
                id="retry-viewer"
                type="button"
                hidden=move || !done() || ui.error(Page::Submit).with(String::is_empty)
                on:click=retry
            >
                "Retry viewer"
            </button>
        </div>
        <p id="archive-notice" role="status" hidden=move || archive_notice.with(String::is_empty)>
            {move || archive_notice.get()}
        </p>
        <p
            id="assembly-status"
            class="sr-only"
            role="status"
            hidden=move || ui.logs.with(Vec::is_empty)
        >
            {move || ui.logs.with(|v| v.last().map(|r| r.1.clone()))}
        </p>
        <ol
            id="assembly-log"
            class="build-log"
            tabindex="0"
            aria-label="Build stages"
            aria-live="polite"
            hidden=move || ui.logs.with(Vec::is_empty)
        >
            {build_log(ui)}
        </ol>
        <p id="build-total" hidden=move || !done()>
            "Total build time: "
            <time id="build-duration">
                {move || ui.result.with(|r| r.as_ref().map(|r| elapsed(r.elapsed)))}
            </time>
        </p>
        <progress
            id="assembly-progress"
            hidden=move || !ui.busy.get() || ui.stage.get() != Page::Submit
        ></progress>
        <p
            id="assembly-error"
            class="error"
            role="alert"
            hidden=move || ui.error(Page::Submit).with(String::is_empty)
        >
            {move || ui.error(Page::Submit).get()}
        </p>
        <ViewerSurface ui panel=Panel::Assembly />
    }
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
