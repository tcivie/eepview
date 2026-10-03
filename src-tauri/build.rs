//! Build script: Tauri code generation, and one permission per IPC command, so the
//! capabilities in `capabilities/` decide which webviews may call each one.

/// Every IPC command of `docs/wiki/ipc-contract.md`.
const COMMANDS: &[&str] = &[
    "tab_new",
    "tab_close",
    "tab_select",
    "tab_move",
    "tab_list",
    "navigate",
    "go_back",
    "go_forward",
    "reload",
    "stop",
    "home",
    "find",
    "find_close",
    "zoom_in",
    "zoom_out",
    "zoom_reset",
    "site_js_set",
    "bookmarks_list",
    "bookmark_add",
    "bookmark_update",
    "bookmark_remove",
    "bookmark_find",
    "bookmarks_export",
    "bookmarks_export_file",
    "bookmarks_import",
    "history_query",
    "history_remove",
    "history_clear",
    "suggest",
    "settings_get",
    "settings_set",
    "router_status",
    "router_stats",
    "connection_pause",
    "connection_resume",
    "router_control",
    "chrome_set_height",
    "platform",
    "chrome_insets",
];

fn main() {
    let manifest = tauri_build::AppManifest::new().commands(COMMANDS);
    let attributes = tauri_build::Attributes::new().app_manifest(manifest);
    if let Err(error) = tauri_build::try_build(attributes) {
        eprintln!("tauri build failed: {error:#}");
        std::process::exit(1);
    }
}
