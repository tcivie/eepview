//! The eepview binary.

// No console window next to the app on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(error) = eepview_lib::run() {
        eprintln!("eepview: {error}");
        std::process::exit(1);
    }
}
