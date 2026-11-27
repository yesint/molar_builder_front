use super::*;

pub(super) fn ligands(ui: Ui) -> impl IntoView {
    view! {
        {move || {
            ui.part
                .get()
                .filter(|p| !p.ligands.is_empty())
                .map(|p| {
                    view! {
                        <p>"Select a ligand to inspect its coordinates and bonds."</p>
                        {p
                            .ligands
                            .into_iter()
                            .enumerate()
                            .map(|(i, n)| {
                                let s = Scene {
                                    path: format!("/api/file/part/{}/{n}", p.id),
                                    name: n,
                                    selection: "all".into(),
                                    style: "ballstick".into(),
                                    ligand: Some(i),
                                    bytes: None,
                                };
                                view! {
                                    <button on:click=move |_| {
                                        ui.viewer.send(Action::Show(s.clone()))
                                    }>{format!("Ligand {}", i + 1)}</button>
                                }
                            })
                            .collect_view()}
                        <button on:click=move |_| {
                            ui.viewer.send(Action::Edit(!ui.editing.get_untracked()))
                        }>
                            {move || {
                                if ui.editing.get() { "Finish editing" } else { "Edit ligand" }
                            }}
                        </button>
                    }
                        .into_any()
                })
                .unwrap_or_else(|| view! { <p>"No ligands selected."</p> }.into_any())
        }}
    }
}
