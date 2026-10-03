/// Starts the eepview application and blocks until it exits.
///
/// # Panics
///
/// Panics if Tauri cannot start the application, for example when the
/// system web view is missing.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
