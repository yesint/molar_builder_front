use super::*;

#[derive(Clone)]
pub(super) struct Download {
    name: String,
    archive: std::sync::Arc<Vec<u8>>,
    coordinates: std::sync::Arc<Vec<u8>>,
    ligands: Vec<std::sync::Arc<Vec<u8>>>,
}

impl Download {
    fn preview(&self, viewer: Viewer) {
        viewer.send(Action::Show(Scene {
            path: String::new(),
            name: "system.cif".into(),
            selection: "all".into(),
            style: "lines".into(),
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
pub(super) fn submit(ui: Ui) -> impl IntoView {
    let summary = move || {
        ui.plan.with(|p| {
            format!(
                "{} lipid species · {} · {}",
                p.membrane.as_ref().map(|m| m.rows.len()).unwrap_or(0),
                if p.partition.is_some() {
                    "Selected molecular components"
                } else {
                    "Membrane only"
                },
                if p.solvent.is_some() {
                    "With solvent"
                } else {
                    "Dry system"
                }
            )
        })
    };
    view! {
        <p>{summary}</p>
        <label>
            "Project name"
            <input
                prop:value=move || ui.plan.with(|p| p.project.clone())
                on:input=move |e| ui.plan.update(|p| p.project = event_target_value(&e))
            />
        </label>
        <button
            class="primary"
            disabled=move || ui.health.get().is_null()
            on:click=move |_| {
                let health = ui.health.get_untracked();
                let popup = if health["mode"] != "local" {
                    match window().open_with_url_and_target("about:blank", "_blank") {
                        Ok(Some(p)) => Some(p),
                        _ => {
                            ui.msg.set("Allow the PayPal checkout window and submit again.".into());
                            return;
                        }
                    }
                } else {
                    None
                };
                ui.task(async move {
                    let result = run_submission(ui, &health, popup.as_ref()).await;
                    if let Some(p) = popup {
                        let _ = p.close();
                    }
                    result
                });
            }
        >
            {move || {
                ui.health
                    .with(|h| {
                        if h["mode"] == "local" {
                            "Build locally".to_string()
                        } else {
                            format!(
                                "Submit and pay USD {}",
                                h["amount"]["value"].as_str().unwrap_or("…"),
                            )
                        }
                    })
            }}
        </button>
        <pre>{move || ui.logs.get()}</pre>
        <div hidden=move || ui.result.with(Option::is_none)>
            <p>"Save the ZIP before closing this tab."</p>
            <button on:click=move |_| {
                ui.task(async move {
                    let result = ui.result.get_untracked().ok_or("No result")?;
                    save_file(&result.name, result.archive.as_slice()).await
                })
            }>"Save ZIP"</button>
            <button on:click=move |_| {
                if let Some(result) = ui.result.get_untracked() {
                    result.preview(ui.viewer);
                }
            }>"View completed system"</button>
        </div>
        {move || {
            ui.result
                .with(|result| {
                    result
                        .as_ref()
                        .map(|r| {
                            (0..r.ligands.len())
                                .map(|index| {
                                    view! {
                                        <button on:click=move |_| {
                                            ui.task(async move {
                                                let result = ui
                                                    .result
                                                    .get_untracked()
                                                    .ok_or("Missing result")?;
                                                save_file(
                                                        &format!("ligand-{}-parameters.zip", index + 1),
                                                        &result.ligands[index],
                                                    )
                                                    .await
                                            })
                                        }>{format!("Save ligand {} parameters", index + 1)}</button>
                                    }
                                })
                                .collect_view()
                        })
                })
        }}
        <a href=api::url("/forcefield/charmm36-feb2026_cgenff-5.0.ff.tgz") download>
            "Download CHARMM36 force field"
        </a>
    }
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
        p.location()
            .replace(url)
            .map_err(|_| "Cannot open checkout")?;
        ui.logs
            .set("Waiting for PayPal approval and signed confirmation.".into());
        let start = js_sys::Date::now();
        loop {
            let v: Value = api::post("/api/payment-status", &json!({}), Some(&k)).await?;
            if v["paid"] == true {
                break;
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
    ui.logs.set("Starting build…".into());
    let done = std::rc::Rc::new(std::cell::Cell::new(false));
    let stop = done.clone();
    let key = k.clone();
    spawn_local(async move {
        while !stop.get() {
            if let Ok(b) = api::bytes("/api/result/progress", Some(&key)).await
                && let Ok(v) = serde_json::from_slice::<Vec<Value>>(&b)
                && !v.is_empty()
                && !stop.get()
            {
                ui.logs.set(
                    v.iter()
                        .filter_map(|r| r[1].as_str())
                        .collect::<Vec<_>>()
                        .join("\n"),
                );
            }
            gloo_timers::future::TimeoutFuture::new(1200).await;
        }
    });
    let r = api::post::<Value>("/api/build", &json!({}), Some(&k)).await;
    done.set(true);
    r?;
    ui.logs.update(|s| s.push_str("\nBuild complete."));
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
    let result = Download {
        name: format!("{}.zip", plan.project),
        archive: archive.into(),
        ligands,
        coordinates: coordinates.into(),
    };
    result.preview(ui.viewer);
    ui.result.set(Some(result));
    api::post::<Value>("/api/received", &json!({}), Some(&k)).await?;
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(serde_json::to_vec(&plan).map_err(|e| e.to_string())?);
    if let Ok(Some(storage)) = window().session_storage() {
        let _ = storage.remove_item(&format!("molar-rust-{mode}-{hash:x}"));
    }
    Ok(())
}
