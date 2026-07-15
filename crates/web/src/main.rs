//! Switchboard console — Leptos CSR entry point.

mod api;
mod app;
mod live;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(app::App);
}
