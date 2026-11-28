use crate::{api, model::Plan};
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use futures_util::StreamExt;
use leptos::prelude::*;
use molar::prelude::*;
use molar_vis_web::{MoleculeEvent, MoleculeHandle, SystemSource, ViewerHandle};
use std::{collections::BTreeMap, io::Cursor, rc::Rc};
use web_sys::HtmlCanvasElement;
#[derive(Clone)]
pub struct Scene {
    pub path: String,
    pub name: String,

    pub selection: String,
    pub ligand: Option<usize>,
    pub layers: Vec<Layer>,
    pub bytes: Option<std::sync::Arc<Vec<u8>>>,
}
pub const PALETTE: [&str; 18] = [
    "#e63232", "#ff8c00", "#f0dc28", "#3cc850", "#3cc8d2", "#3c6ef0", "#9650dc", "#dc3cc8",
    "#f082b4", "#1e9696", "#aadc28", "#283c96", "#f5f5f5", "#c8c8c8", "#8c8c8c", "#505050",
    "#191919", "#965a32",
];
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub role: &'static str,
    pub selection: String,
    pub style: &'static str,
    pub color: &'static str,
}
impl Layer {
    pub fn new(
        role: &'static str,
        selection: String,
        style: &'static str,
        color: &'static str,
    ) -> Self {
        Self {
            role,
            selection,
            style,
            color,
        }
    }
}
pub struct Preview {
    pub bytes: Option<std::sync::Arc<Vec<u8>>>,
    pub path: String,
    pub name: String,
    pub layers: Vec<Layer>,
}
struct LoadedPreview {
    molecule: u64,
    layers: Vec<PreviewLayer>,
}
struct PreviewLayer {
    settings: Layer,
    rep: u64,
    visible: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Load,
    Protein,
    Ligand,
    Assembly,
}
pub enum Action {
    Show(Scene),
    Preview(Panel, Vec<Preview>),
    Edit(bool),
    Save(futures_channel::oneshot::Sender<Result<(), String>>),
}
impl Action {
    fn panel(&self) -> Option<Panel> {
        match self {
            Self::Preview(panel, _) => Some(*panel),
            Self::Show(scene) if scene.ligand.is_some() => Some(Panel::Ligand),
            Self::Show(_) => Some(Panel::Assembly),
            Self::Edit(_) | Self::Save(_) => None,
        }
    }
}
#[derive(Clone, Copy)]
pub struct Viewer {
    pub status: [RwSignal<&'static str>; 4],
    tx: StoredValue<UnboundedSender<Action>, LocalStorage>,
}
impl Viewer {
    pub fn new() -> (Self, UnboundedReceiver<Action>) {
        let (tx, rx) = unbounded();
        (
            Self {
                tx: StoredValue::new_local(tx),
                status: std::array::from_fn(|_| RwSignal::new("idle")),
            },
            rx,
        )
    }
    pub fn send(self, a: Action) {
        let _ = self.tx.with_value(|tx| tx.unbounded_send(a));
    }
    pub async fn save(self) -> Result<(), String> {
        let (tx, rx) = futures_channel::oneshot::channel();
        self.send(Action::Save(tx));
        rx.await.map_err(|_| "Viewer closed".to_string())?
    }
}
struct Current {
    molecule: MoleculeHandle,

    ligand: Option<usize>,
    partition: Option<String>,
    initial_cif: Option<String>,
}

impl Current {
    async fn save(&self, plan: RwSignal<Plan>) -> Result<(), String> {
        if let Some(index) = self.ligand {
            let cif = self.molecule.export_cif().await?;
            if self.initial_cif.as_ref() != Some(&cif) {
                plan.update(|plan| {
                    if plan.partition == self.partition {
                        plan.edits.insert(index, cif);
                    }
                });
            }
        }
        Ok(())
    }
}

struct Controller {
    canvas: HtmlCanvasElement,
    host: Option<ViewerHandle>,
    current: Option<Current>,
    previews: BTreeMap<String, LoadedPreview>,
    plan: RwSignal<Plan>,
    editing: RwSignal<bool>,
}

impl Controller {
    async fn apply(&mut self, action: Action) -> Result<(), String> {
        match action {
            Action::Save(reply) => {
                let result = self.save().await;
                let _ = reply.send(result.clone());
                result?;
            }
            Action::Preview(_, models) => self.preview(models).await?,
            Action::Edit(enabled) => {
                if let Some(current) = &self.current
                    && current.ligand.is_some()
                {
                    current.molecule.set_editing(enabled).await?;
                }
            }
            Action::Show(scene) => self.show(scene).await?,
        }
        Ok(())
    }

    async fn save(&self) -> Result<(), String> {
        if let Some(current) = &self.current {
            current.save(self.plan).await?;
        }
        Ok(())
    }

    async fn start(&mut self) -> Result<(), String> {
        if self.host.is_none() {
            let host = ViewerHandle::start(self.canvas.clone()).await?;
            host.with_app(|app| {
                app.set_ui_visibility(false, true);
                app.set_theme("light")?;
                app.set_background_solid([1., 1., 1.]);
                Ok(())
            })
            .await?;
            self.host = Some(host);
        }
        Ok(())
    }
    async fn preview(&mut self, models: Vec<Preview>) -> Result<(), String> {
        if models.is_empty() && self.host.is_none() {
            return Ok(());
        }
        self.start().await?;
        let host = self.host.as_ref().unwrap();
        let replace = self.current.is_some()
            || self
                .previews
                .keys()
                .any(|key| !models.iter().any(|m| &m.path == key));
        if replace {
            self.save().await?;
        }
        let oriented = models.iter().any(|m| m.name == "membrane_planes.cif");
        let mut updates = Vec::with_capacity(models.len());
        for mut model in models {
            model
                .layers
                .retain(|layer| !layer.selection.is_empty() && layer.selection != "not all");
            let loaded = !replace && self.previews.contains_key(&model.path);
            let system = if loaded {
                None
            } else {
                if model.layers.is_empty() {
                    continue;
                }
                let bytes = match model.bytes.take() {
                    Some(bytes) => bytes.as_ref().clone(),
                    None => api::bytes(&model.path, None).await?,
                };
                Some(read_system(&model.name, bytes)?)
            };
            updates.push((model, system));
        }

        let previews = &mut self.previews;
        host.update_now(|app| {
            if replace {
                app.clear();
                previews.clear();
                self.current = None;
                self.editing.set(false);
            }
            let mut added = false;
            for (model, system) in updates {
                if let Some(system) = system {
                    let source = Box::new(SystemSource(Rc::new(system)));
                    let index = app.add_shared_molecule(source, model.name)?;
                    let molecule = app.molecule_ids()[index];
                    for rep in app.rep_ids(molecule)? {
                        app.remove_rep_by_id(molecule, rep)?;
                    }
                    previews.insert(
                        model.path.clone(),
                        LoadedPreview {
                            molecule,
                            layers: vec![],
                        },
                    );
                    added = true;
                }
                let entry = previews.get_mut(&model.path).expect("preview was loaded");
                for old in &mut entry.layers {
                    if old.visible
                        && !model
                            .layers
                            .iter()
                            .any(|layer| layer.role == old.settings.role)
                    {
                        app.set_rep_visible(entry.molecule, old.rep, false)?;
                        old.visible = false;
                    }
                }
                for layer in model.layers {
                    if let Some(old) = entry
                        .layers
                        .iter_mut()
                        .find(|old| old.settings.role == layer.role)
                    {
                        if old.settings != layer {
                            let (mi, ri) = app.resolve_rep(entry.molecule, old.rep)?;
                            if old.settings.selection != layer.selection {
                                app.set_rep_selection_text(mi, ri, &layer.selection)?;
                            }
                            if old.settings.style != layer.style {
                                app.set_rep_style(mi, ri, layer.style)?;
                            }
                            if old.settings.color != layer.color {
                                app.set_rep_color(mi, ri, layer.color)?;
                            }
                            old.settings = layer;
                        }
                        if !old.visible {
                            app.set_rep_visible(entry.molecule, old.rep, true)?;
                            old.visible = true;
                        }
                    } else {
                        let rep = app.add_styled_rep(
                            entry.molecule,
                            &layer.selection,
                            layer.style,
                            layer.color,
                        )?;
                        entry.layers.push(PreviewLayer {
                            settings: layer,
                            rep,
                            visible: true,
                        });
                    }
                }
            }
            if added {
                app.reset_view();
                if oriented {
                    app.rotate_view(0., 70.);
                }
            }
            Ok(())
        })
    }

    async fn show(&mut self, scene: Scene) -> Result<(), String> {
        self.save().await?;
        let saved = scene
            .ligand
            .and_then(|index| self.plan.with_untracked(|p| p.edits.get(&index).cloned()));
        let final_system = scene.bytes.is_some();
        let (name, bytes) = if let Some(cif) = saved {
            ("ligand.cif".into(), cif.into_bytes())
        } else if let Some(bytes) = scene.bytes {
            (scene.name, bytes.as_ref().clone())
        } else {
            (scene.name, api::bytes(&scene.path, None).await?)
        };
        self.start().await?;
        self.previews.clear();
        let host = self.host.as_ref().unwrap();
        host.clear().await?;
        self.current = None;
        self.editing.set(false);
        let mut layers = vec![];
        let molecule = if scene.ligand.is_some() {
            host.add_editable(name, bytes).await?
        } else {
            let system = read_system(&name, bytes)?;
            if final_system {
                layers = scene
                    .layers
                    .into_iter()
                    .filter(|layer| {
                        !layer.selection.is_empty()
                            && layer.selection != "not all"
                            && system.select(&layer.selection).is_ok()
                    })
                    .collect();
            }
            host.add_system(name, Rc::new(system)).await?
        };
        for representation in molecule.representations().await? {
            representation.remove().await?;
        }
        if !final_system {
            layers.push(Layer::new(
                "ligand",
                scene.selection,
                "ballstick",
                "element",
            ));
            molecule.set_editing(false).await?;
        }
        let id = molecule.id();
        host.with_app(move |app| {
            for layer in layers {
                let rep = app.add_styled_rep(id, &layer.selection, layer.style, layer.color)?;
                if layer.style == "ballstick" {
                    let (mi, ri) = app.resolve_rep(id, rep)?;
                    app.set_ballstick_bonds(mi, ri, 0.15, 0.5)?;
                }
            }
            if final_system {
                let index = app.resolve_molecule(id)?;
                app.set_molecule_box_visible(index, true)?;
            }
            app.reset_view();
            Ok(())
        })
        .await?;
        let partition = self.plan.with_untracked(|p| p.partition.clone());
        let initial_cif = if let Some(index) = scene.ligand {
            let baseline = molecule.export_cif().await?;
            watch_edits(
                molecule.clone(),
                index,
                partition.clone(),
                self.plan,
                self.editing,
            )
            .await?;
            Some(baseline)
        } else {
            None
        };
        self.current = Some(Current {
            molecule,
            ligand: scene.ligand,
            partition,
            initial_cif,
        });
        Ok(())
    }
}

fn read_system(name: &str, bytes: Vec<u8>) -> Result<System, String> {
    let extension = name.rsplit('.').next().unwrap_or("cif");
    let mut reader =
        FileHandler::from_reader(extension, Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let (topology, state) = reader.read().map_err(|e| e.to_string())?;
    System::new(topology, state).map_err(|e| e.to_string())
}
async fn watch_edits(
    molecule: MoleculeHandle,
    index: usize,
    partition: Option<String>,
    plan: RwSignal<Plan>,
    editing: RwSignal<bool>,
) -> Result<(), String> {
    let mut events = molecule.events().await?;
    leptos::task::spawn_local(async move {
        while let Some(event) = events.next().await {
            if !plan.with_untracked(|p| p.partition == partition) {
                break;
            }
            let capture = match event {
                MoleculeEvent::Editing { enabled, .. } => {
                    editing.set(enabled);
                    enabled
                }
                MoleculeEvent::Changed { .. } => true,
                MoleculeEvent::Removed { .. } => break,
            };
            if capture && let Ok(cif) = molecule.export_cif().await {
                plan.update(|p| {
                    if p.partition == partition {
                        p.edits.insert(index, cif);
                    }
                });
            }
        }
    });
    Ok(())
}

pub async fn serve(
    canvas: HtmlCanvasElement,
    mut actions: UnboundedReceiver<Action>,
    viewer: Viewer,
    plan: RwSignal<Plan>,
    report_error: impl Fn(Panel, String),
    editing: RwSignal<bool>,
) {
    let mut controller = Controller {
        canvas,
        host: None,
        current: None,
        previews: Default::default(),
        plan,
        editing,
    };
    let mut pending = None;
    loop {
        let mut action = match pending.take() {
            Some(action) => action,
            None => match actions.next().await {
                Some(action) => action,
                None => break,
            },
        };
        // Each preview describes the entire scene. Preserve save, edit and show ordering.
        while matches!(action, Action::Preview(..)) {
            match actions.try_recv() {
                Ok(next @ Action::Preview(..)) => action = next,
                other => {
                    pending = other.ok();
                    break;
                }
            }
        }
        let panel = action.panel();
        if let Some(panel) = panel {
            viewer.status[panel as usize].set("updating");
        }
        if let Err(error) = controller.apply(action).await {
            if let Some(p) = panel {
                viewer.status[p as usize].set("unavailable");
            }
            report_error(panel.unwrap_or(Panel::Ligand), error);
        } else if let Some(p) = panel {
            viewer.status[p as usize].set("rendered");
        }
    }
}
