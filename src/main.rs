//! AuroraCast — cross-platform entry point.
//!
//! One binary, three targets, selected at build time by Cargo features:
//!   dx serve   --platform web                  (hot-reload dev server)
//!   dx serve   --platform desktop              (native window)
//!   dx serve   --platform ios | android        (simulator / device)
//!   dx bundle  --platform <web|desktop|ios|android> --release
//!
//! If you prefer plain cargo for quick iteration:
//!   cargo run --features desktop
//!   cargo run --features web          (usually run via `dx serve` instead)

#![allow(non_snake_case)]
// Desktop release: hide the console window on Windows.
#![cfg_attr(all(feature = "desktop", not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod state;
mod weather;

use dioxus::prelude::*;

/// The shell HTML — inlined CSS + map.js + Leaflet CDN. Served on every
/// platform (web via index.html, desktop via `with_custom_index`, mobile
/// likewise). Dioxus replaces the `<div id="main"></div>` with the rendered
/// app tree; anything else is left as-is.
const INDEX_HTML: &str = include_str!("../index.html");

fn main() {
    // `dioxus::launch` selects the platform launcher automatically based on
    // the feature that's enabled (web / desktop / mobile). Platform-specific
    // config (window size, rootname, etc.) is applied below before launch.
    #[cfg(feature = "web")]
    {
        let cfg = dioxus::web::Config::new().rootname("main");
        LaunchBuilder::web().with_cfg(cfg).launch(app::App);
    }

    #[cfg(feature = "desktop")]
    {
        use dioxus::desktop::{Config, WindowBuilder, LogicalSize};
        let cfg = Config::new()
            .with_custom_index(INDEX_HTML.to_string())
            .with_window(
                WindowBuilder::new()
                    .with_title("AuroraCast")
                    .with_inner_size(LogicalSize::new(1280.0, 820.0))
                    .with_min_inner_size(LogicalSize::new(380.0, 640.0))
                    .with_visible(true)
                    .with_focused(true)
                    .with_decorations(true)
                    .with_resizable(true),
            );
        LaunchBuilder::desktop().with_cfg(cfg).launch(app::App);
    }

    #[cfg(feature = "mobile")]
    {
        // On mobile, Dioxus wraps the webview in a native shell. The `dx`
        // CLI handles the iOS / Android packaging; we just launch the app.
        LaunchBuilder::mobile().launch(app::App);
    }

    #[cfg(not(any(feature = "web", feature = "desktop", feature = "mobile")))]
    compile_error!("Enable one feature: --features web|desktop|mobile");
}
