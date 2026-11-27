mod checkout;
mod components;
mod glycan;
mod ligands;
mod membrane;
mod protein;
mod solvent;

use crate::{
    api,
    model::*,
    viewer::{Action, Scene, Viewer},
};
use leptos::{prelude::*, task::spawn_local};
use serde_json::{Value, json};
use std::future::Future;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;

#[derive(Clone, Copy)]
struct Ui {
    plan: RwSignal<Plan>,
    stage: RwSignal<usize>,
    busy: RwSignal<bool>,
    msg: RwSignal<String>,
    uploads: RwSignal<Vec<Upload>>,
    choices: RwSignal<Vec<Selection>>,
    active_source: RwSignal<String>,
    part: RwSignal<Option<Partition>>,
    catalog: RwSignal<Vec<Lipid>>,
    health: RwSignal<Value>,
    viewer: Viewer,
    editing: RwSignal<bool>,
    saved_membrane: StoredValue<Membrane>,
    saved_solvent: StoredValue<Solvent>,
    result: RwSignal<Option<checkout::Download>>,
    logs: RwSignal<String>,
}
impl Ui {
    fn task(self, f: impl Future<Output = Result<(), String>> + 'static) {
        if self.busy.get_untracked() {
            return;
        }
        self.busy.set(true);
        self.msg.set(String::new());
        spawn_local(async move {
            if let Err(e) = f.await {
                self.msg.set(e)
            }
            self.busy.set(false)
        });
    }
    fn navigate(self, step: usize) {
        self.task(async move {
            self.viewer.save().await?;
            let previous = self.stage.get_untracked();
            if previous == 0 && step != 0 {
                self.partition_components().await?;
            }
            if step == 5 {
                self.plan
                    .get_untracked()
                    .validate(&self.catalog.get_untracked())?;
            } else if previous == 3
                && step > previous
                && let Some(membrane) = self.plan.get_untracked().membrane
            {
                membrane.normalized(&self.catalog.get_untracked())?;
            }
            self.stage.set(step);
            self.preview(step);
            Ok(())
        })
    }
    fn preview(self, step: usize) {
        let mut scene = None;
        let p = self.part.get_untracked();
        match step {
            0 => {
                if let Some(up) = self.uploads.with_untracked(|v| v.first().cloned()) {
                    self.preview_upload(&up);
                    return;
                }
            }
            1 => {
                if let Some(p) = &p
                    && !p.protein.is_null()
                {
                    scene = Some((
                        format!("/api/file/part/{}/protein.cif", p.id),
                        "protein.cif".into(),
                        if p.protein["coarse_grained"] == true {
                            "vdw"
                        } else {
                            "cartoon"
                        },
                        None,
                    ));
                }
            }
            2 => {
                if let Some(p) = &p
                    && let Some(n) = p.ligands.first()
                {
                    scene = Some((
                        format!("/api/file/part/{}/{n}", p.id),
                        n.clone(),
                        "ballstick",
                        Some(0),
                    ));
                }
            }
            _ => {}
        }
        if let Some((path, name, style, ligand)) = scene {
            self.viewer.send(Action::Show(Scene {
                path,
                name,
                style: style.into(),
                selection: "all".into(),
                ligand,
                bytes: None,
            }));
        } else {
            self.viewer.send(Action::Clear);
        }
    }
    fn preview_upload(self, u: &Upload) {
        self.active_source.set(u.id.clone());
        let f = if u.format == "sdf" {
            "source.sdf"
        } else {
            "structure.cif"
        };
        let s = self.choices.with_untracked(|v| {
            v.iter()
                .find(|x| x.id == u.id)
                .map(combined)
                .unwrap_or_default()
        });
        self.viewer.send(Action::Show(Scene {
            path: format!("/api/file/upload/{}/{f}", u.id),
            name: f.into(),
            style: if u.format == "sdf" {
                "ballstick"
            } else if u.report["coarse_grained"] == true {
                "vdw"
            } else {
                "lines"
            }
            .into(),
            selection: s,
            ligand: None,
            bytes: None,
        }));
    }
    fn change_choices(self, id: &str, protein: bool, value: String) {
        self.choices.update(|v| {
            if let Some(c) = v.iter_mut().find(|s| s.id == id) {
                if protein {
                    c.protein_selection = value
                } else {
                    c.ligand_selection = value
                }
                if self.active_source.get_untracked() == id {
                    self.viewer.send(Action::Select(combined(c)));
                }
            }
        });
        self.invalidate_components();
    }
    fn invalidate_components(self) {
        self.part.set(None);
        self.plan.update(|p| {
            p.partition = None;
            p.protein = Protein {
                orient: true,
                ..Default::default()
            };
            p.edits.clear()
        });
    }
    fn advance(self) {
        self.navigate(self.stage.get_untracked() + 1);
    }
    async fn partition_components(self) -> Result<(), String> {
        let choices = self
            .choices
            .get_untracked()
            .into_iter()
            .filter(|s| !s.protein_selection.is_empty() || !s.ligand_selection.is_empty())
            .collect::<Vec<_>>();
        if !choices.is_empty() && self.part.get_untracked().is_none() {
            let p: Partition = api::post("/api/partition", &choices, None).await?;
            self.plan.update(|v| {
                v.partition = Some(p.id.clone());
                v.protein.ss_bonds = p.protein["ss_bonds"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|b| serde_json::from_value(b["residues"].clone()).ok())
                    .collect();
            });
            self.part.set(Some(p));
        }
        Ok(())
    }
}
fn combined(s: &Selection) -> String {
    [&s.protein_selection, &s.ligand_selection]
        .iter()
        .filter(|s| !s.is_empty())
        .map(|s| format!("({s})"))
        .collect::<Vec<_>>()
        .join(" or ")
}
fn input(e: &web_sys::Event) -> HtmlInputElement {
    e.target().unwrap().unchecked_into()
}
const STEPS: [&str; 6] = [
    "Components",
    "Protein",
    "Ligands",
    "Membrane",
    "Solvent",
    "Submit",
];
#[component]
pub fn Builder() -> impl IntoView {
    let (viewer, rx) = Viewer::new();
    let ui = Ui {
        plan: RwSignal::new(Plan::default()),
        stage: RwSignal::new(0),
        busy: RwSignal::new(false),
        msg: RwSignal::new(String::new()),
        uploads: RwSignal::new(vec![]),
        choices: RwSignal::new(vec![]),
        active_source: RwSignal::new(String::new()),
        part: RwSignal::new(None),
        catalog: RwSignal::new(vec![]),
        health: RwSignal::new(Value::Null),
        viewer,
        editing: RwSignal::new(false),
        saved_membrane: StoredValue::new(Plan::default().membrane.unwrap()),
        saved_solvent: StoredValue::new(Plan::default().solvent.unwrap()),
        result: RwSignal::new(None),
        logs: RwSignal::new(String::new()),
    };
    let canvas = NodeRef::<leptos::html::Canvas>::new();
    canvas.on_load(move |c| spawn_local(crate::viewer::serve(c, rx, ui.plan, ui.msg, ui.editing)));
    spawn_local(async move {
        match api::get::<Value>("/api/health").await {
            Ok(v) => ui.health.set(v),
            Err(e) => ui.msg.set(e),
        }
        match gloo_net::http::Request::get("./membrane-assets/catalog.json")
            .send()
            .await
        {
            Ok(r) => match r.json::<Catalogue>().await {
                Ok(c) => ui.catalog.set(c.lipids),
                Err(e) => ui.msg.set(e.to_string()),
            },
            Err(e) => ui.msg.set(e.to_string()),
        }
    });
    let outcome = window().location().search().unwrap_or_default();
    let returned = outcome.starts_with("?checkout=");
    view! {
        <header>
            <strong>"MolAR Builder"</strong>
            <span>
                {move || {
                    ui.health
                        .with(|v| match v["mode"].as_str() {
                            Some("local") => "Local builder",
                            Some("sandbox") => "PayPal Sandbox",
                            Some("production") => "Cloud builder",
                            _ => "Connecting",
                        })
                }}
            </span>
        </header>
        <main hidden=returned>
            <nav aria-label="Build steps">
                {STEPS
                    .iter()
                    .enumerate()
                    .map(|(i, &s)| {
                        view! {
                            <button
                                class:active=move || ui.stage.get() == i
                                disabled=move || ui.busy.get()
                                on:click=move |_| ui.navigate(i)
                            >
                                {s}
                            </button>
                        }
                    })
                    .collect_view()}
            </nav>
            <section class="workspace">
                <article>
                    <h1>{move || STEPS[ui.stage.get()]}</h1>
                    <fieldset disabled=move || {
                        ui.busy.get()
                    }>
                        {move || match ui.stage.get() {
                            0 => components::components(ui).into_any(),
                            1 => protein::protein(ui).into_any(),
                            2 => ligands::ligands(ui).into_any(),
                            3 => membrane::membrane(ui).into_any(),
                            4 => solvent::solvent(ui).into_any(),
                            _ => checkout::submit(ui).into_any(),
                        }}
                    </fieldset>
                    <p role="alert" hidden=move || ui.msg.get().is_empty()>
                        {move || ui.msg.get()}
                    </p>
                    <button
                        class="primary"
                        hidden=move || ui.stage.get() == 5
                        disabled=move || ui.busy.get()
                        on:click=move |_| ui.advance()
                    >
                        "Continue"
                    </button>
                </article>
                <aside hidden=move || !matches!(ui.stage.get(), 0..=2 | 5)>
                    <canvas node_ref=canvas aria-label="Molecular structure"></canvas>
                </aside>
            </section>
            <p role="status">{move || if ui.busy.get() { "Working…" } else { "" }}</p>
        </main>
        <section hidden=!returned>
            <h1>"PayPal checkout"</h1>
            <p>
                {if outcome.contains("cancel") {
                    "Checkout was cancelled."
                } else {
                    "Return to the builder tab to finish the submission."
                }}
            </p>
        </section>
    }
}
