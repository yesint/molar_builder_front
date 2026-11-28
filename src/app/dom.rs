use leptos::prelude::*;
pub fn paged<T, V>(items: Vec<T>, render: impl Fn(T) -> V + Clone + Send + 'static) -> impl IntoView
where
    T: Clone + Send + Sync + 'static,
    V: IntoView + 'static,
{
    const PAGE_SIZE: usize = 100;
    let pages = items.len().div_ceil(PAGE_SIZE);
    let items = StoredValue::new(items);
    let page = RwSignal::new(0usize);
    view! {
        <div>
            {move || {
                let start = page.get() * PAGE_SIZE;
                items
                    .with_value(|v| {
                        v.iter()
                            .skip(start)
                            .take(PAGE_SIZE)
                            .cloned()
                            .map(render.clone())
                            .collect_view()
                    })
            }}
            {(pages > 1)
                .then(move || {
                    view! {
                        <div class="actions">
                            <button
                                class="secondary compact"
                                disabled=move || page.get() == 0
                                on:click=move |_| page.update(|n| *n = n.saturating_sub(1))
                            >
                                "Previous"
                            </button>
                            <button
                                class="secondary compact"
                                disabled=move || page.get() + 1 == pages
                                on:click=move |_| page.update(|n| *n = (*n + 1).min(pages - 1))
                            >
                                "Next"
                            </button>
                        </div>
                    }
                })}
        </div>
    }
}

#[derive(Clone, Copy)]
pub(super) struct Modal(pub NodeRef<leptos::html::Dialog>);
impl Modal {
    pub fn new() -> Self {
        Self(NodeRef::new())
    }
    pub fn open(self) {
        if let Some(element) = self.0.get_untracked() {
            let _ = element.show_modal();
        }
    }
    pub fn close(self) {
        if let Some(element) = self.0.get_untracked() {
            element.close();
        }
    }
}
