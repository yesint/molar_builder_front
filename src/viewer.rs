use crate::{api, model::Plan};
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use futures_util::StreamExt;
use leptos::prelude::*;
use molar::prelude::*;
use molar_vis_web::{MoleculeEvent, MoleculeHandle, RepresentationHandle, ViewerHandle};
use std::{io::Cursor, rc::Rc};
use web_sys::HtmlCanvasElement;
#[derive(Clone)]
pub struct Scene {
    pub path: String,
    pub name: String,
    pub style: String,
    pub selection: String,
    pub ligand: Option<usize>,
    pub bytes: Option<std::sync::Arc<Vec<u8>>>,
}
pub enum Action {
    Show(Scene),
    Clear,
    Select(String),
    Edit(bool),
    Save(futures_channel::oneshot::Sender<Result<(), String>>),
}
#[derive(Clone, Copy)]
pub struct Viewer {
    tx: StoredValue<UnboundedSender<Action>, LocalStorage>,
}
impl Viewer {
    pub fn new() -> (Self, UnboundedReceiver<Action>) {
        let (tx, rx) = unbounded();
        (
            Self {
                tx: StoredValue::new_local(tx),
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
    representation: RepresentationHandle,
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
            Action::Clear => {
                if let Some(host) = &self.host {
                    host.clear().await?;
                }
                self.current = None;
                self.editing.set(false);
            }
            Action::Edit(enabled) => {
                if let Some(current) = &self.current
                    && current.ligand.is_some()
                {
                    current.molecule.set_editing(enabled).await?;
                }
            }
            Action::Select(selection) => {
                if let Some(current) = &self.current {
                    if !selection.is_empty() {
                        current
                            .representation
                            .select_text(selection.clone())
                            .await?;
                    }
                    current
                        .representation
                        .set_visible(!selection.is_empty())
                        .await?;
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
        let host = self.host.as_ref().unwrap();
        host.clear().await?;
        self.current = None;
        self.editing.set(false);
        let mut layers = vec![];
        let molecule = if scene.ligand.is_some() {
            host.add_editable(name, bytes).await?
        } else {
            let extension = name.rsplit('.').next().unwrap_or("cif");
            let mut reader = FileHandler::from_reader(extension, Cursor::new(bytes))
                .map_err(|e| e.to_string())?;
            let (topology, state) = reader.read().map_err(|e| e.to_string())?;
            let system = System::new(topology, state).map_err(|e| e.to_string())?;
            if final_system {
                layers = assembly_layers(&system, self.plan);
            }
            host.add_system(name, Rc::new(system)).await?
        };
        for representation in molecule.representations().await? {
            representation.remove().await?;
        }
        let visible = !scene.selection.is_empty() && layers.is_empty();
        let selection = if scene.selection.is_empty() {
            "all".into()
        } else {
            scene.selection
        };
        let color = if scene.ligand.is_some() {
            "element"
        } else {
            "chain"
        };
        let representation = molecule
            .add_rep(selection, scene.style, color.into())
            .await?;
        representation.set_visible(visible).await?;
        if final_system {
            let id = molecule.id();
            host.with_app(move |app| {
                for (selection, style, color) in layers {
                    app.add_styled_rep(id, &selection, style, color)?;
                }
                let index = app.resolve_molecule(id)?;
                app.set_molecule_box_visible(index, true)?;
                app.reset_view();
                Ok(())
            })
            .await?;
        }
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
            representation,
            ligand: scene.ligand,
            partition,
            initial_cif,
        });
        Ok(())
    }
}

fn assembly_layers(
    system: &System,
    plan: RwSignal<Plan>,
) -> Vec<(String, &'static str, &'static str)> {
    let lipids = plan.with_untracked(|plan| {
        plan.membrane
            .as_ref()
            .map(|m| m.rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>())
            .unwrap_or_default()
    });
    let names: std::collections::BTreeSet<_> = system
        .iter_atoms()
        .map(|a| a.get_resname().to_owned())
        .collect();
    let mut layers = Vec::new();
    let mut protein = Vec::new();
    for name in names {
        let (style, color) = if name == "SOL" {
            ("lines", "element")
        } else if matches!(name.as_str(), "SOD" | "POT" | "MG" | "CAL" | "CLA") {
            ("vdw", "element")
        } else if lipids.contains(&name) {
            ("licorice", "resname")
        } else if name
            .strip_prefix('L')
            .is_some_and(|s| s.len() == 4 && s.bytes().all(|c| c.is_ascii_digit()))
        {
            ("ballstick", "element")
        } else {
            protein.push(name);
            continue;
        };
        layers.push((format!("resname {name}"), style, color));
    }
    if !protein.is_empty() {
        layers.push((format!("resname {}", protein.join(" ")), "cartoon", "chain"));
    }
    layers
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
            match event {
                MoleculeEvent::Editing { enabled, .. } => editing.set(enabled),
                MoleculeEvent::Changed { .. } => {
                    if let Ok(cif) = molecule.export_cif().await {
                        plan.update(|p| {
                            if p.partition == partition {
                                p.edits.insert(index, cif);
                            }
                        });
                    }
                }
                MoleculeEvent::Removed { .. } => break,
            }
        }
    });
    Ok(())
}

pub async fn serve(
    canvas: HtmlCanvasElement,
    mut actions: UnboundedReceiver<Action>,
    plan: RwSignal<Plan>,
    message: RwSignal<String>,
    editing: RwSignal<bool>,
) {
    let mut controller = Controller {
        canvas,
        host: None,
        current: None,
        plan,
        editing,
    };
    while let Some(action) = actions.next().await {
        if let Err(error) = controller.apply(action).await {
            message.set(error);
        }
    }
}
