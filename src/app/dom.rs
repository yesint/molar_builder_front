use leptos::prelude::*;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{
    Element, Event, EventTarget, HtmlDialogElement, HtmlElement, HtmlInputElement,
    HtmlSelectElement,
};

pub fn el(id: &str) -> Element {
    document()
        .get_element_by_id(id)
        .unwrap_or_else(|| panic!("Missing page element {id}"))
}
pub fn field(id: &str) -> HtmlInputElement {
    el(id).unchecked_into()
}
pub fn select(id: &str) -> HtmlSelectElement {
    el(id).unchecked_into()
}
pub fn dialog(id: &str) -> HtmlDialogElement {
    el(id).unchecked_into()
}
pub fn flag(element: &Element, name: &str, enabled: bool) {
    element.toggle_attribute_with_force(name, enabled).unwrap();
}
pub fn visible(id: &str, show: bool) {
    flag(&el(id), "hidden", !show);
}
pub fn enabled(id: &str, enable: bool) {
    flag(&el(id), "disabled", !enable);
}
pub fn label(id: &str, text: &str) {
    el(id).set_text_content(Some(text));
}
pub fn notice(id: &str, text: &str) {
    label(id, text);
    visible(id, !text.is_empty());
}
pub fn focus(id: &str) {
    if let Some(e) = document().get_element_by_id(id) {
        let _ = e.unchecked_into::<HtmlElement>().focus();
    }
}
pub fn listen(target: &EventTarget, event: &str, f: impl FnMut(Event) + 'static) {
    let cb = Closure::wrap(Box::new(f) as Box<dyn FnMut(Event)>);
    target
        .add_event_listener_with_callback(event, cb.as_ref().unchecked_ref())
        .unwrap();
    // Static template controls live for the entire lifetime of the client.
    cb.forget();
}
pub fn on(id: &str, event: &str, f: impl FnMut(Event) + 'static) {
    listen(el(id).as_ref(), event, f);
}
pub fn click(id: &str, f: impl Fn() + 'static) {
    on(id, "click", move |_| f());
}
pub fn mount<V: IntoView + 'static>(id: &str, f: impl FnOnce() -> V + 'static) {
    leptos::mount::mount_to(el(id).unchecked_into(), f).forget();
}
pub fn options(id: &str, values: impl IntoIterator<Item = (String, String)>) {
    let select = select(id);
    let previous = select.value();
    select.set_text_content(None);
    for (value, label) in values {
        let option = document().create_element("option").unwrap();
        option.set_attribute("value", &value).unwrap();
        option.set_text_content(Some(&label));
        select.append_child(&option).unwrap();
    }
    select.set_value(&previous);
    if select.selected_index() < 0 {
        select.set_selected_index(0);
    }
}

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
                            <button class="secondary compact" disabled=move || page.get() + 1>
                                =pages on:click=move |_|page.update(|n|*n=(*n+1).min(pages-1))>
                                "Next"
                            </button>
                        </div>
                    }
                })}
        </div>
    }
}
