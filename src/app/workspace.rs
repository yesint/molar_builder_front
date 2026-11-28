use super::*;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Page {
    Components,
    Protein,
    Ligands,
    Membrane,
    Solvent,
    Submit,
}
impl Page {
    const ALL: [Self; 6] = [
        Self::Components,
        Self::Protein,
        Self::Ligands,
        Self::Membrane,
        Self::Solvent,
        Self::Submit,
    ];
    pub(super) fn slug(self) -> &'static str {
        match self {
            Self::Components => "load",
            Self::Protein => "proteins",
            Self::Ligands => "ligands",
            Self::Membrane => "membrane",
            Self::Solvent => "solvent",
            Self::Submit => "submit",
        }
    }
    fn title(self) -> &'static str {
        match self {
            Self::Components => "1. Components",
            Self::Protein => "2. Protein",
            Self::Ligands => "3. Ligands",
            Self::Membrane => "4. Membrane",
            Self::Solvent => "5. Solvent",
            Self::Submit => "6. Submit",
        }
    }
    fn help(self) -> &'static str {
        match self {
            Self::Components => {
                "Load your files and choose which proteins and other molecules to keep. If you only want a membrane, go straight to the Membrane step."
            }
            Self::Protein => {
                "Choose any changes you want to make to your protein. If you are adding a membrane, select \"Orient\" to align the protein with it."
            }
            Self::Membrane => {
                "Choose the membrane size and the mix of lipids on each side. To skip the membrane, unselect \"Add membrane\"."
            }
            Self::Ligands => {
                "Review the small molecules you chose. You can edit their atoms and bonds. Make sure each molecule has all its hydrogens before you continue."
            }
            Self::Solvent => {
                "Choose how much water to add and what type and concentration of salt to use. Select \"Neutralize\" to balance the total charge. To skip adding water and salt, unselect \"Add solvent\"."
            }
            Self::Submit => {
                "Review your choices and start the build. When it finishes, select Save ZIP to download your files before closing the tab."
            }
        }
    }
    fn check_id(self) -> &'static str {
        match self {
            Self::Components => "load-ready",
            Self::Protein => "protein-ready",
            Self::Ligands => "ligand-ready",
            Self::Membrane => "membrane-ready",
            Self::Solvent => "solvent-ready",
            Self::Submit => "assembly-ready",
        }
    }
    fn allowed(self, ui: Ui) -> bool {
        match self {
            Self::Protein => {
                ui.has_protein() || ui.uploads.with(|v| v.iter().any(|u| u.format != "sdf"))
            }
            Self::Ligands => ui.has_ligands(),
            _ => true,
        }
    }
}
pub(super) fn route(ui: Ui) {
    let hash = window().location().hash().unwrap_or_default();
    let slug = match hash.trim_start_matches('#') {
        "assembly" => "submit",
        slug => slug,
    };
    let page = Page::ALL
        .into_iter()
        .find(|p| p.slug() == slug && p.allowed(ui))
        .unwrap_or(Page::Components);
    let target = format!("#{}", page.slug());
    if !hash.is_empty()
        && hash != target
        && let Ok(history) = window().history()
    {
        let _ = history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&target));
    }
    ui.stage.set(page);
}
#[component]
pub(super) fn StageHeading(ui: Ui, page: Page) -> impl IntoView {
    view! {
        <div class="stage-heading">
            <h2
                id=format!("{}-heading", page.slug())
                tabindex="-1"
                node_ref=ui.headings[page as usize]
            >
                {page.title()}
            </h2>
            <button
                class="secondary help-button"
                type="button"
                aria-label=format!("Help for {}", page.title().split_once(". ").unwrap().1)
                aria-haspopup="dialog"
                aria-controls="stage-help"
                on:click=move |_| ui.help.open()
            >
                "?"
            </button>
        </div>
    }
}
#[component]
pub(super) fn BetaTerms(ui: Ui) -> impl IntoView {
    view! {
        <h3>"Public beta"</h3>
        <p>
            "MolAR builder is in public beta. To discourage automated submissions and cover server costs, each submitted system costs "
            <strong data-build-price="">{move || ui.price()}</strong>
            ". This fee is your contribution as a volunteer beta tester. No particular result is guaranteed. No payment credentials are collected or stored by MolAR builder."
        </p>
        <p>
            "By clicking “Submit”, you agree to use the builder and its output at your own risk. Both are provided without warranty of any kind, to the extent permitted by law."
        </p>
        <p>
            "Generated systems are free to use for any purpose under "
            <a href="https://creativecommons.org/licenses/by/4.0/" target="_blank" rel="noopener">
                "CC BY 4.0"
            </a>". Data will be used to debug the builder and may also be used to train AI models."
        </p>
        <p>"If you don’t agree with these terms, do not press “Submit”."</p>
    }
}
#[component]
pub(super) fn ViewerSurface(ui: Ui, panel: Panel) -> impl IntoView {
    let (id, label) = match panel {
        Panel::Load => ("load-viewer", "Loaded structure"),
        Panel::Protein => ("protein-viewer", "Protein structure"),
        Panel::Ligand => ("ligand-viewer", "Ligand structure"),
        Panel::Assembly => ("assembly-viewer", "Assembled system"),
    };
    view! {
        <div
            id=id
            class="structure-viewer"
            aria-label=label
            node_ref=ui.panels[panel as usize]
            hidden=move || !ui.panel_visible(panel)
            data-state=move || ui.viewer.status[panel as usize].get()
        ></div>
    }
}
#[component]
fn Navigation(ui: Ui) -> impl IntoView {
    view! {
        <aside class="project-tree">
            <h2>"Project"</h2>
            <nav aria-label="Project sections">
                <ul>
                    {Page::ALL
                        .into_iter()
                        .map(move |page| {
                            view! {
                                <li>
                                    <a
                                        href=move || {
                                            page.allowed(ui).then(|| format!("#{}", page.slug()))
                                        }
                                        data-page=page.slug()
                                        aria-disabled=move || (!page.allowed(ui)).then_some("true")
                                        aria-current=move || {
                                            (ui.stage.get() == page).then_some("page")
                                        }
                                    >
                                        {page.title()}
                                        " "
                                        <span
                                            id=page.check_id()
                                            class="tree-check"
                                            aria-label="Complete"
                                            hidden=move || !ui.confirmed.get()[page as usize]
                                        >
                                            "✓"
                                        </span>
                                    </a>
                                </li>
                            }
                        })
                        .collect_view()}
                </ul>
            </nav>
        </aside>
    }
}
#[component]
fn Stage(ui: Ui, page: Page, children: Children) -> impl IntoView {
    view! {
        <section
            id=format!("page-{}", page.slug())
            aria-labelledby=format!("{}-heading", page.slug())
            hidden=move || ui.stage.get() != page
        >
            {children()}
        </section>
    }
}
#[component]
pub(super) fn Workspace(ui: Ui, standby: NodeRef<leptos::html::Div>) -> impl IntoView {
    let about = Modal::new();
    let returned = checkout::checkout_return();
    view! {
        <div id="viewer-standby" node_ref=standby inert aria-hidden="true"></div>
        <main class="membrane-app">
            <header class="app-heading">
                <h1>"MolAR builder"</h1>
                <button
                    id="open-about"
                    class="secondary help-button"
                    type="button"
                    aria-label="About MolAR builder"
                    aria-haspopup="dialog"
                    aria-controls="about-builder"
                    on:click=move |_| about.open()
                >
                    "?"
                </button>
            </header>
            <p id="checkout-return" class="notice" role="status" hidden=returned.is_none()>
                {returned}
            </p>
            <div class="project-layout">
                <Navigation ui />
                <div class="project-content">
                    <Stage ui page=Page::Components>
                        <components::ComponentsPage ui />
                    </Stage>
                    <Stage ui page=Page::Protein>
                        <protein::ProteinPage ui />
                    </Stage>
                    <Stage ui page=Page::Ligands>
                        <ligands::LigandsPage ui />
                    </Stage>
                    <Stage ui page=Page::Membrane>
                        <membrane::MembranePage ui />
                    </Stage>
                    <Stage ui page=Page::Solvent>
                        <solvent::SolventPage ui />
                    </Stage>
                    <Stage ui page=Page::Submit>
                        <checkout::SubmitPage ui />
                    </Stage>
                </div>
            </div>
        </main>
        <dialog
            id="stage-help"
            aria-labelledby="stage-help-heading"
            aria-describedby="stage-help-text"
            node_ref=ui.help.0
        >
            <form method="dialog" class="dialog-title">
                <h2 id="stage-help-heading" tabindex="-1" autofocus>
                    {move || ui.stage.get().title()}
                </h2>
                <button type="submit" class="secondary" aria-label="Close stage help">
                    "×"
                </button>
            </form>
            <p id="stage-help-text">{move || ui.stage.get().help()}</p>
        </dialog>
        <dialog id="about-builder" aria-labelledby="about-heading" node_ref=about.0>
            <div class="dialog-title about-title">
                <h2 id="about-heading" tabindex="-1" autofocus>
                    "About MolAR builder"
                </h2>
                <button
                    type="submit"
                    form="about-actions"
                    class="secondary"
                    aria-label="Close About dialog"
                >
                    "×"
                </button>
            </div>
            <p>
                "MolAR builder is an independent clean-room implementation of a CHARMM-GUI-like system builder for MD simulations and molecular modeling. It uses no code or copyrighted assets from CHARMM-GUI. The implementation is based entirely on published research papers and open-source resources."
            </p>
            <p>
                "Under the hood, the builder uses the "
                <a href="https://github.com/yesint/molar" target="_blank" rel="noopener">
                    "molar"
                </a>" molecular modeling library and the "
                <a href="https://github.com/yesint/molar_vis" target="_blank" rel="noopener">
                    "molar_vis"
                </a>" visualizer. The frontend of the service is "
                <a
                    href="https://github.com/yesint/molar_builder_front"
                    target="_blank"
                    rel="noopener"
                >
                    "open source"
                </a>". The server backend is currently proprietary."
            </p>
            <div id="public-beta-terms">
                <BetaTerms ui />
            </div>
            <form id="about-actions" method="dialog">
                <button type="submit" class="secondary compact">
                    "Close"
                </button>
            </form>
        </dialog>
    }
}
