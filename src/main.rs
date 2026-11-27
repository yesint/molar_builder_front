#[cfg(target_arch = "wasm32")]
mod api;
#[cfg(target_arch = "wasm32")]
mod app;
mod model;
#[cfg(target_arch = "wasm32")]
mod viewer;
#[cfg(target_arch = "wasm32")]
fn main() {
    leptos::mount::mount_to_body(app::Builder);
}
#[cfg(not(target_arch = "wasm32"))]
fn main() {}
