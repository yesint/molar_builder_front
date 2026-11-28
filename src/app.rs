mod checkout;
mod dom;
use dom::*;
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
    files: RwSignal<Vec<components::Source>>,
    confirmed: RwSignal<[bool; 6]>,
    ligand: RwSignal<usize>,
    part: RwSignal<Option<Partition>>,
    catalog: RwSignal<Vec<Lipid>>,
    health: RwSignal<Value>,
    viewer: Viewer,
    editing: RwSignal<bool>,
    saved_membrane: StoredValue<Membrane>,
    result: RwSignal<Option<checkout::Download>>,
    logs: RwSignal<Vec<(f64, String)>>,
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
    fn has_protein(self) -> bool {
        self.part
            .with(|p| p.as_ref().is_some_and(|p| !p.protein.is_null()))
    }
    fn has_ligands(self) -> bool {
        self.part
            .with(|p| p.as_ref().is_some_and(|p| !p.ligands.is_empty()))
    }
    fn set_done(self, step: usize, done: bool) {
        self.confirmed.update(|flags| flags[step] = done);
    }
    fn invalidate_components(self) {
        self.part.set(None);
        self.confirmed.update(|flags| {
            for i in [0, 1, 2, 5] {
                flags[i] = false;
            }
        });
        self.ligand.set(0);
        self.plan.update(|p| {
            p.partition = None;
            p.edits.clear();
            p.protein = Plan::default().protein;
        });
    }
    fn components_ready(self) -> bool {
        self.uploads.with(Vec::is_empty) || self.confirmed.get()[0]
    }
    fn ready(self) -> bool {
        let confirmed = self.confirmed.get();
        !self.health.get().is_null()
            && self.components_ready()
            && (!self.has_protein() || confirmed[1])
            && (!self.has_ligands() || confirmed[2])
            && if self.plan.with(|p| p.membrane.is_some()) {
                confirmed[3]
            } else {
                self.has_protein() || self.has_ligands()
            }
    }
    fn go(self, step: usize) {
        let _ = window().location().set_hash(PAGES[step]);
    }
    fn show_ligand(self, index: usize) {
        if let Some(p) = self.part.get_untracked()
            && let Some(index) = (0..p.ligands.len())
                .find(|&i| i == index && !ligands::water(&p, i))
                .or_else(|| (0..p.ligands.len()).find(|&i| !ligands::water(&p, i)))
            && let Some(name) = p.ligands.get(index)
        {
            self.ligand.set(index);
            self.viewer.send(Action::Show(Scene {
                path: format!("/api/file/part/{}/{name}", p.id),
                name: name.clone(),
                layers: vec![],
                selection: "all".into(),
                ligand: Some(index),
                bytes: None,
            }));
        }
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
const PAGES: [&str; 6] = [
    "load", "proteins", "ligands", "membrane", "solvent", "submit",
];
fn route(ui: Ui) {
    let hash = window().location().hash().unwrap_or_default();
    let page = match hash.trim_start_matches('#') {
        "assembly" => "submit",
        p => p,
    };
    let stage = PAGES.iter().position(|&s| s == page).unwrap_or(0);
    let stage = match stage {
        1 if !ui.has_protein() && !ui.uploads.with(|v| v.iter().any(|u| u.format != "sdf")) => 0,
        2 if !ui.has_ligands() => 0,
        i => i,
    };
    if !hash.is_empty() && hash != format!("#{}", PAGES[stage]) {
        let _ = window().history().unwrap().replace_state_with_url(
            &wasm_bindgen::JsValue::NULL,
            "",
            Some(&format!("#{}", PAGES[stage])),
        );
    }
    ui.stage.set(stage);
}
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
        files: RwSignal::new(vec![]),
        confirmed: RwSignal::new([false; 6]),
        ligand: RwSignal::new(0),
        part: RwSignal::new(None),
        catalog: RwSignal::new(vec![]),
        health: RwSignal::new(Value::Null),
        viewer,
        editing: RwSignal::new(false),
        saved_membrane: StoredValue::new(Plan::default().membrane.unwrap()),
        result: RwSignal::new(None),
        logs: RwSignal::new(vec![]),
    };
    let canvas: web_sys::HtmlCanvasElement = document()
        .create_element("canvas")
        .unwrap()
        .unchecked_into();
    canvas
        .set_attribute("aria-label", "Molecular structure")
        .unwrap();
    spawn_local(crate::viewer::serve(
        canvas.clone(),
        rx,
        ui.plan,
        ui.msg,
        ui.editing,
    ));
    setup(ui, canvas);
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
}
fn setup(ui: Ui, canvas: web_sys::HtmlCanvasElement) {
    components::setup(ui);
    protein::setup(ui);
    ligands::setup(ui);
    membrane::setup(ui);
    solvent::setup(ui);
    checkout::setup(ui);
    click("open-about", || {
        let _ = dialog("about-builder").show_modal();
    });
    listen(document().as_ref(), "click", move |event| {
        let Some(target) = event
            .target()
            .and_then(|v| v.dyn_into::<web_sys::Element>().ok())
        else {
            return;
        };
        if let Some(help) = target.get_attribute("data-help") {
            let stage = ui.stage.get_untracked();
            label(
                "stage-help-heading",
                &el(&format!("{}-heading", PAGES[stage]))
                    .text_content()
                    .unwrap_or_default(),
            );
            label("stage-help-text", &help);
            let _ = dialog("stage-help").show_modal();
        }
        if let Some(id) = target.get_attribute("data-close") {
            dialog(&id).close();
        }
    });
    route(ui);
    listen(window().as_ref(), "hashchange", move |_| {
        route(ui);
        focus(&format!("{}-heading", PAGES[ui.stage.get_untracked()]));
    });
    Effect::new(move |_| {
        let stage = ui.stage.get();
        let flags = ui.confirmed.get();
        for (i, page) in PAGES.iter().enumerate() {
            let a = document()
                .query_selector(&format!("[data-page={page}]"))
                .unwrap()
                .unwrap();
            let allowed = match i {
                1 => ui.has_protein() || ui.uploads.with(|v| v.iter().any(|u| u.format != "sdf")),
                2 => ui.has_ligands(),
                _ => true,
            };
            if allowed {
                a.set_attribute("href", &format!("#{page}")).unwrap();
                a.remove_attribute("aria-disabled").unwrap();
            } else {
                a.remove_attribute("href").unwrap();
                a.set_attribute("aria-disabled", "true").unwrap();
            }
            visible(&format!("page-{page}"), i == stage);
            if i == stage {
                a.set_attribute("aria-current", "page").unwrap();
            } else {
                a.remove_attribute("aria-current").unwrap();
            }
            visible(
                [
                    "load-ready",
                    "protein-ready",
                    "ligand-ready",
                    "membrane-ready",
                    "solvent-ready",
                    "assembly-ready",
                ][i],
                flags[i],
            );
        }
    });
    Effect::new(move |_| {
        let stage = ui.stage.get();
        let dest = match stage {
            0 => "load-viewer",
            1 => "protein-viewer",
            2 => "ligand-viewer",
            5 => "assembly-viewer",
            _ => return,
        };
        if canvas.parent_element().is_none_or(|p| p.id() != dest) {
            el(dest).append_child(&canvas).unwrap();
        }
        match stage {
            0 => components::preview(ui),
            1 => protein::preview(ui),
            2 => ui.show_ligand(ui.ligand.get_untracked()),
            5 => {
                if let Some(r) = ui.result.get_untracked() {
                    r.preview(ui.viewer);
                }
            }
            _ => (),
        }
    });
    Effect::new(move |_| {
        let id = [
            "load-error",
            "protein-error",
            "ligand-error",
            "composition-error",
            "assembly-error",
            "assembly-error",
        ][ui.stage.get()];
        let msg = ui.msg.get();
        if !msg.is_empty() || id != "composition-error" {
            notice(id, &msg);
        }
    });
}
