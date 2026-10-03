//! eepview: a browser that opens I2P sites only.

pub mod core;
pub mod hover;
pub mod layout;
pub mod nav;
pub mod net;
pub mod session;
pub mod shortcuts;
pub mod store;
pub mod suggest;
pub mod tabs;
pub mod types;

/// Starts the eepview application and blocks until it exits.
///
/// # Errors
///
/// Fails when Tauri cannot start, for example when the system web view is missing.
pub fn run() -> tauri::Result<()> {
    tauri::Builder::default().run(tauri::generate_context!())
}
