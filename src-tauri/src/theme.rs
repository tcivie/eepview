// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The surface colour of the active theme, read from the design tokens in `theme.css`.

use crate::store::settings::Theme;

/// The `--color-surface` token of the theme as `[red, green, blue, alpha]`. `system_dark` says
/// whether the operating system is in dark mode; it counts only for [`Theme::System`].
#[must_use]
pub fn surface_rgba(_theme: Theme, _system_dark: bool) -> [u8; 4] {
    [0, 0, 0, 0]
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIGHT: [u8; 4] = [0xfa, 0xf7, 0xf0, 255];
    const DARK: [u8; 4] = [0x12, 0x34, 0x2b, 255];

    // UX1-2: the light theme uses the light `--color-surface` token.
    #[test]
    fn ux1_2_light_theme_is_the_light_surface() {
        assert_eq!(surface_rgba(Theme::Light, false), LIGHT);
    }

    // UX1-2: the dark theme uses the dark `--color-surface` token.
    #[test]
    fn ux1_2_dark_theme_is_the_dark_surface() {
        assert_eq!(surface_rgba(Theme::Dark, false), DARK);
    }

    // UX1-2: an explicit theme ignores the operating system.
    #[test]
    fn ux1_2_explicit_themes_ignore_the_system() {
        assert_eq!(surface_rgba(Theme::Light, true), LIGHT);
        assert_eq!(surface_rgba(Theme::Dark, true), DARK);
    }

    // UX1-2: the system theme follows the operating system.
    #[test]
    fn ux1_2_system_theme_follows_the_system() {
        assert_eq!(surface_rgba(Theme::System, false), LIGHT);
        assert_eq!(surface_rgba(Theme::System, true), DARK);
    }

    // UX1-2: the stated values are the ones in theme.css.
    #[test]
    fn ux1_2_values_match_the_design_tokens() {
        let css = include_str!("../../src/ui/theme.css");
        assert!(css.contains("--color-surface: #faf7f0;"));
        assert!(css.contains("--color-surface: #12342b;"));
    }
}
