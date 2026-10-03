// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The theme surface colour on the native side: the window behind the webviews, and a new
//! tab webview before its first paint. Without it the engine shows white while a page loads.

use tauri::window::Color;
use tauri::{AppHandle, Manager, Runtime, Theme as OsTheme};

use super::state::{lock, shared};
use crate::theme::surface_rgba;

/// The `--color-surface` of the saved theme; "system" follows the operating system.
pub fn color<R: Runtime>(app: &AppHandle<R>) -> Color {
    let theme = lock(&shared(app).core).settings().theme;
    let system_dark = app
        .get_window("main")
        .and_then(|w| w.theme().ok())
        .is_some_and(|t| t == OsTheme::Dark);
    let [red, green, blue, alpha] = surface_rgba(theme, system_dark);
    Color(red, green, blue, alpha)
}

/// Paints the main window with the surface colour. Call it at start and when the theme
/// changes.
pub fn paint_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_window("main") {
        let _ = window.set_background_color(Some(color(app)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::testing::bare;

    #[test]
    fn the_surface_is_opaque_and_painting_needs_no_window() {
        let app = bare();
        assert_eq!(color(app.handle()).3, 255);
        paint_window(app.handle());
    }
}
