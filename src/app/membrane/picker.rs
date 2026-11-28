use super::*;

#[derive(Clone, Copy)]
pub(super) struct PickerState {
    modal: Modal,
    query: RwSignal<String>,
    preview: RwSignal<Option<Lipid>>,
    image_error: RwSignal<bool>,
    search: NodeRef<leptos::html::Input>,
    pub(super) button: NodeRef<leptos::html::Button>,
    pub(super) focus_row: StoredValue<Option<String>>,
    pub(super) hover: RwSignal<Option<Lipid>>,
    pub(super) position: RwSignal<(f64, f64)>,
    pub(super) popup: NodeRef<leptos::html::Aside>,
}
impl PickerState {
    pub(super) fn new() -> Self {
        Self {
            modal: Modal::new(),
            query: RwSignal::new(String::new()),
            preview: RwSignal::new(None),
            image_error: RwSignal::new(false),
            search: NodeRef::new(),
            button: NodeRef::new(),
            focus_row: StoredValue::new(None),
            hover: RwSignal::new(None),
            position: RwSignal::new((0., 0.)),
            popup: NodeRef::new(),
        }
    }
    fn show(self, lipid: Lipid) {
        self.image_error.set(false);
        self.preview.set(Some(lipid));
    }
    pub(super) fn open(self) {
        self.query.set(String::new());
        self.hover.set(None);
        self.preview.set(None);
        self.modal.open();
        if let Some(input) = self.search.get_untracked() {
            let _ = input.focus();
        }
    }
    pub(super) fn show_hover(self, lipid: Lipid, button: NodeRef<leptos::html::Button>) {
        let Some(button) = button.get_untracked() else {
            return;
        };
        self.hover.set(Some(lipid));
        let rect = button.get_bounding_client_rect();
        request_animation_frame(move || {
            if let Some(host) = self.popup.get_untracked() {
                let size = host.get_bounding_client_rect();
                let w = window()
                    .inner_width()
                    .ok()
                    .and_then(|v| v.as_f64())
                    .unwrap_or_default();
                let h = window()
                    .inner_height()
                    .ok()
                    .and_then(|v| v.as_f64())
                    .unwrap_or_default();
                self.position.set((
                    (rect.right() + 12.).min(w - size.width() - 8.).max(8.),
                    rect.top().min(h - size.height() - 8.).max(8.),
                ));
            }
        });
    }
}
fn categories(
    field: &'static str,
    control: &'static str,
    list: Memo<Vec<String>>,
    active: Memo<String>,
    signal: RwSignal<String>,
    state: PickerState,
) -> impl IntoView {
    move || {
        list.get()
            .into_iter()
            .map(|name| {
                let key = StoredValue::new(name.clone());
                view! {
                    <button
                        class="secondary picker-choice"
                        data-family=(field == "family").then(|| name.clone())
                        data-subfamily=(field == "subfamily").then(|| name.clone())
                        aria-controls=control
                        aria-pressed=move || (active.get() == key.get_value()).to_string()
                        on:click=move |_| {
                            signal.set(key.get_value());
                            state.preview.set(None);
                        }
                    >
                        {name.clone()}
                    </button>
                }
            })
            .collect_view()
    }
}
#[component]
pub(super) fn LipidPicker(ui: Ui, state: PickerState) -> impl IntoView {
    let query = state.query;
    let family = RwSignal::new(String::new());
    let sub = RwSignal::new(String::new());
    let matches = Memo::new(move |_| {
        let query = query.get().to_lowercase();
        ui.catalog.with(|c| {
            c.iter()
                .filter(|l| {
                    let hay = format!(
                        "{} {} {} {} {} {} {}",
                        l.id, l.name, l.family, l.subfamily, l.group, l.chemical_subfamily, l.tails
                    )
                    .to_lowercase();
                    query.split_whitespace().all(|term| hay.contains(term))
                })
                .cloned()
                .collect::<Vec<_>>()
        })
    });
    let families = Memo::new(move |_| unique(matches.get().into_iter().map(|l| l.family)));
    let current = Memo::new(move |_| chosen(families.get(), family.get()));
    let subs = Memo::new(move |_| {
        unique(
            matches
                .get()
                .into_iter()
                .filter(|l| l.family == current.get())
                .map(|l| l.subfamily),
        )
    });
    let subcurrent = Memo::new(move |_| chosen(subs.get(), sub.get()));
    let filter = move |e: leptos::ev::Event| {
        let raw = event_target_value(&e);
        let q = raw.trim().to_lowercase();
        let c = ui.catalog.get_untracked();
        if let Some(l) = c
            .iter()
            .find(|l| l.id.eq_ignore_ascii_case(&q))
            .or_else(|| c.iter().find(|l| l.subfamily.eq_ignore_ascii_case(&q)))
        {
            family.set(l.family.clone());
            sub.set(l.subfamily.clone());
        }
        query.set(raw);
        state.preview.set(None);
    };
    view! {
        <dialog
            id="lipid-picker"
            aria-labelledby="picker-heading"
            node_ref=state.modal.0
            on:close=move |_| state.preview.set(None)
        >
            <div class="picker-heading">
                <h2 id="picker-heading">"Add lipids"</h2>
                <button
                    id="close-picker"
                    class="secondary"
                    aria-label="Close lipid picker"
                    on:click=move |_| state.modal.close()
                >
                    "×"
                </button>
            </div>
            <label for="lipid-search">
                "Search by family, subfamily, molecule, or tail composition"
            </label>
            <input
                id="lipid-search"
                type="search"
                placeholder="e.g. POPC or 18:0"
                autocomplete="off"
                node_ref=state.search
                prop:value=move || query.get()
                on:input=filter
            />
            <div class="picker-scroll">
                <div class="picker-layout">
                    <div class="picker-column">
                        <h3>"Family"</h3>
                        <div id="lipid-families" role="group" aria-label="Lipid family">
                            {categories(
                                "family",
                                "lipid-subfamilies",
                                families,
                                current,
                                family,
                                state,
                            )}
                        </div>
                    </div>
                    <div class="picker-column">
                        <h3>"Subfamily"</h3>
                        <div id="lipid-subfamilies" role="group" aria-label="Lipid subfamily">
                            {categories("subfamily", "picker-lipids", subs, subcurrent, sub, state)}
                        </div>
                    </div>
                    <div class="picker-column">
                        <h3>"Molecules"</h3>
                        <div id="picker-lipids" role="group" aria-label="Molecules">
                            {move || {
                                matches
                                    .get()
                                    .into_iter()
                                    .filter(|l| {
                                        l.family == current.get() && l.subfamily == subcurrent.get()
                                    })
                                    .map(|l| picker_button(ui, l, state))
                                    .collect_view()
                            }}
                        </div>
                    </div>
                    <aside class="structure-preview" aria-label="Lipid chemical structure">
                        <h3>"Picture"</h3>
                        <div
                            id="preview-content"
                            hidden=move || state.preview.with(Option::is_none)
                        >
                            <h4 id="preview-name">
                                {move || state.preview.with(|l| l.as_ref().map(|l| l.name.clone()))}
                            </h4>
                            <p
                                id="preview-tails"
                                class="small"
                                hidden=move || {
                                    state
                                        .preview
                                        .with(|l| l.as_ref().is_none_or(|l| l.tails.is_empty()))
                                }
                            >
                                {move || {
                                    state.preview.with(|l| l.as_ref().map(|l| l.tails.clone()))
                                }}
                            </p>
                            <img
                                id="preview-image"
                                width="180"
                                height="350"
                                src=move || {
                                    state.preview.with(|l| l.as_ref().map(|l| l.image.clone()))
                                }
                                alt=move || {
                                    state
                                        .preview
                                        .with(|l| {
                                            l.as_ref().map(|l| format!("{} chemical structure", l.name))
                                        })
                                }
                                hidden=move || state.image_error.get()
                                on:error=move |_| state.image_error.set(true)
                            />
                            <p
                                id="preview-error"
                                class="small"
                                hidden=move || !state.image_error.get()
                            >
                                "The structure image could not be loaded."
                            </p>
                        </div>
                    </aside>
                </div>
            </div>
            <p id="no-lipids" role="status" hidden=move || !matches.with(Vec::is_empty)>
                "No lipids match your search."
            </p>
        </dialog>
    }
}
fn picker_button(ui: Ui, l: Lipid, state: PickerState) -> impl IntoView {
    let lipid = StoredValue::new(l.clone());
    let added = ui.plan.with(|p| {
        p.membrane
            .as_ref()
            .is_some_and(|m| m.rows.iter().any(|r| r.id == l.id))
    });
    view! {
        <button
            class="secondary catalog-lipid"
            data-lipid=l.id.clone()
            aria-label=format!(
                "{} {}{}",
                if added { "Already added" } else { "Add" },
                l.name,
                if l.tails.is_empty() { String::new() } else { format!(", {}", l.tails) },
            )
            aria-disabled=added.to_string()
            on:mouseenter=move |_| state.show(lipid.get_value())
            on:focus=move |_| state.show(lipid.get_value())
            on:mouseleave=move |_| state.preview.set(None)
            on:blur=move |_| state.preview.set(None)
            on:click=move |_| {
                if added {
                    return;
                }
                let id = lipid.get_value().id;
                update(
                    ui,
                    |m| {
                        m.rows
                            .push(Row {
                                id: id.clone(),
                                lower: 0.,
                                upper: 0.,
                            })
                    },
                );
                state.focus_row.set_value(Some(id));
                state.modal.close();
            }
        >
            <span>
                {l.name.clone()}
                {(!l.tails.is_empty())
                    .then(|| view! { <span class="small lipid-tail">{l.tails.clone()}</span> })}
            </span>
            <span aria-hidden="true">{if added { "Added" } else { "+" }}</span>
        </button>
    }
}
fn unique(items: impl Iterator<Item = String>) -> Vec<String> {
    let mut out = vec![];
    for s in items {
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}
fn chosen(items: Vec<String>, s: String) -> String {
    if items.contains(&s) {
        s
    } else {
        items.into_iter().next().unwrap_or_default()
    }
}
