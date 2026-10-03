// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

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
    "window_fullscreen",
];

fn main() {
    let manifest = tauri_build::AppManifest::new().commands(COMMANDS);
    // tauri-build would embed the Windows manifest into the app binary only; test binaries
    // then fail to start (STATUS_ENTRYPOINT_NOT_FOUND). Embed one manifest everywhere.
    let windows = tauri_build::WindowsAttributes::new_without_app_manifest();
    let attributes = tauri_build::Attributes::new()
        .app_manifest(manifest)
        .windows_attributes(windows);
    embed_windows_manifest();
    if let Err(error) = tauri_build::try_build(attributes) {
        eprintln!("tauri build failed: {error:#}");
        std::process::exit(1);
    }
}

/// Links `windows-app-manifest.xml` into every MSVC executable of the crate, tests included.
fn embed_windows_manifest() {
    let target = std::env::var("TARGET").unwrap_or_default();
    if !target.ends_with("windows-msvc") {
        return;
    }
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let manifest = std::path::Path::new(&dir).join("windows-app-manifest.xml");
    println!("cargo:rerun-if-changed=windows-app-manifest.xml");
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
}
