mod checkout;
mod dom;
use dom::*;
mod components;
mod glycan;
mod ligands;
mod membrane;
mod protein;
mod solvent;
mod workspace;
use workspace::*;

use crate::{
    api,
    model::*,
    viewer::{Action, Panel, Scene, Viewer},
};
use leptos::{prelude::*, task::spawn_local};
use serde_json::{Value, json};
use std::future::Future;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;

#[derive(Clone, Copy)]
struct Ui {
    plan: RwSignal<Plan>,
    stage: RwSignal<Page>,
    help: Modal,
    headings: [NodeRef<leptos::html::H2>; 6],
    panels: [NodeRef<leptos::html::Div>; 4],
    busy: RwSignal<bool>,
    errors: [RwSignal<String>; 6],
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
    fn paid(self) -> bool {
        self.health
            .with(|h| matches!(h["mode"].as_str(), Some("sandbox" | "production")))
    }
    fn price(self) -> String {
        self.health
            .with(|h| format!("USD {}", h["amount"]["value"].as_str().unwrap_or("0.50")))
    }
    fn has_editable_ligands(self) -> bool {
        self.part.with(|p| {
            p.as_ref()
                .is_some_and(|p| (0..p.ligands.len()).any(|i| !ligands::water(p, i)))
        })
    }
    fn panel_visible(self, panel: Panel) -> bool {
        match panel {
            Panel::Load => self.files.with(|files| !files.is_empty()),
            Panel::Protein => true,
            Panel::Ligand => self.has_editable_ligands(),
            Panel::Assembly => self.confirmed.get()[Page::Submit as usize],
        }
    }
    fn error(self, page: Page) -> RwSignal<String> {
        self.errors[page as usize]
    }
    fn task(self, f: impl Future<Output = Result<(), String>> + 'static) {
        if self.busy.get_untracked() {
            return;
        }
        self.busy.set(true);
        let error = self.error(self.stage.get_untracked());
        error.set(String::new());
        spawn_local(async move {
            if let Err(e) = f.await {
                error.set(e)
            }
            self.busy.set(false)
        });
    }
    fn has_protein(self) -> bool {
        self.part
            .with(|p| p.as_ref().is_some_and(|p| p.protein.is_some()))
    }
    fn has_ligands(self) -> bool {
        self.part
            .with(|p| p.as_ref().is_some_and(|p| !p.ligands.is_empty()))
    }
    fn set_done(self, step: Page, done: bool) {
        self.confirmed.update(|flags| flags[step as usize] = done);
    }
    fn invalidate_build(self) {
        self.set_done(Page::Submit, false);
        self.logs.set(Vec::new());
        self.error(Page::Submit).set(String::new());
    }
    fn invalidate_components(self) {
        self.invalidate_build();
        self.part.set(None);
        self.confirmed.update(|flags| {
            for i in [Page::Components, Page::Protein, Page::Ligands] {
                flags[i as usize] = false;
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
        self.uploads.with(Vec::is_empty) || self.confirmed.get()[Page::Components as usize]
    }
    fn ready(self) -> bool {
        let confirmed = self.confirmed.get();
        !self.health.get().is_null()
            && self.components_ready()
            && (!self.has_protein() || confirmed[Page::Protein as usize])
            && (!self.has_ligands() || confirmed[Page::Ligands as usize])
            && if self.plan.with(|p| p.membrane.is_some()) {
                confirmed[Page::Membrane as usize]
            } else {
                self.has_protein() || self.has_ligands()
            }
    }
    fn go(self, step: Page) {
        let _ = window().location().set_hash(step.slug());
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
            let mut p: Partition = api::post("/api/partition", &choices, None).await?;
            if let Some(protein) = &mut p.protein {
                protein.mark_duplicate_numbers();
            }
            self.plan.update(|v| {
                v.partition = Some(p.id.clone());
                v.protein.ss_bonds = p
                    .protein
                    .iter()
                    .flat_map(|p| p.ss_bonds.iter().map(|b| b.residues))
                    .collect();
            });
            self.part.set(Some(p));
        }
        Ok(())
    }
}
#[component]
pub fn Builder() -> impl IntoView {
    let (viewer, rx) = Viewer::new();
    let ui = Ui {
        plan: RwSignal::new(Plan::default()),
        stage: RwSignal::new(Page::Components),
        help: Modal::new(),
        headings: std::array::from_fn(|_| NodeRef::new()),
        panels: std::array::from_fn(|_| NodeRef::new()),
        busy: RwSignal::new(false),
        errors: std::array::from_fn(|_| RwSignal::new(String::new())),
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
        viewer,
        ui.plan,
        move |panel, error| {
            let page = match panel {
                Panel::Load => Page::Components,
                Panel::Protein => Page::Protein,
                Panel::Ligand => Page::Ligands,
                Panel::Assembly => Page::Submit,
            };
            ui.error(page).set(error);
        },
        ui.editing,
    ));
    route(ui);
    let routing = window_event_listener(leptos::ev::hashchange, move |_| {
        route(ui);
        request_animation_frame(move || {
            if let Some(heading) = ui.headings[ui.stage.get_untracked() as usize].get_untracked() {
                let _ = heading.focus();
            }
        });
    });
    on_cleanup(move || routing.remove());
    let standby = NodeRef::<leptos::html::Div>::new();
    Effect::new(move |_| {
        let panel = match ui.stage.get() {
            Page::Components => Some(Panel::Load),
            Page::Protein => Some(Panel::Protein),
            Page::Ligands => Some(Panel::Ligand),
            Page::Submit => Some(Panel::Assembly),
            _ => None,
        };
        let target = panel
            .filter(|&p| ui.panel_visible(p))
            .map(|p| ui.panels[p as usize])
            .unwrap_or(standby);
        if let Some(host) = target.get()
            && canvas.parent_node().as_ref() != Some(host.as_ref())
        {
            let _ = host.append_child(&canvas);
        }
    });
    Effect::new(move |_| match ui.stage.get() {
        Page::Components => components::preview(ui),
        Page::Protein => protein::preview(ui),
        Page::Ligands => ui.show_ligand(ui.ligand.get_untracked()),
        Page::Submit => {
            if let Some(r) = ui.result.get_untracked() {
                r.preview(ui.viewer);
            }
        }
        _ => (),
    });
    spawn_local(async move {
        match api::get::<Value>("/api/health").await {
            Ok(v) => ui.health.set(v),
            Err(e) => {
                ui.error(Page::Components).set(e.clone());
                ui.error(Page::Submit).set(e);
            }
        }
        match gloo_net::http::Request::get("./membrane-assets/catalog.json")
            .send()
            .await
        {
            Ok(r) => match r.json::<Catalogue>().await {
                Ok(c) => ui.catalog.set(c.lipids),
                Err(e) => ui.error(Page::Membrane).set(e.to_string()),
            },
            Err(e) => ui.error(Page::Membrane).set(e.to_string()),
        }
    });
    view! { <Workspace ui standby /> }
}
