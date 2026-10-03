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

    const CSS: &str = include_str!("../../src/ui/theme.css");

    /// The `--color-surface` value of the block that starts with `header`, as RGBA.
    fn token(header: &str) -> Option<[u8; 4]> {
        let block = CSS.split_once(header)?.1.split_once('}')?.0;
        let value = block.split_once("--color-surface:")?.1.split_once(';')?.0;
        let hex = value.trim().strip_prefix('#')?;
        let channel = |at: usize| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok();
        Some([channel(0)?, channel(2)?, channel(4)?, 255])
    }

    fn light() -> [u8; 4] {
        token(":root {").expect("the light block defines --color-surface")
    }

    fn dark() -> [u8; 4] {
        token(":root[data-theme=\"dark\"] {").expect("the dark block defines --color-surface")
    }

    // UX1-2: both blocks of theme.css define the token the tests below read.
    #[test]
    fn ux1_2_both_blocks_define_the_surface_token() {
        assert!(token(":root {").is_some());
        assert!(token(":root[data-theme=\"dark\"] {").is_some());
    }

    // UX1-2: the light theme uses the light `--color-surface` token.
    #[test]
    fn ux1_2_light_theme_is_the_light_surface() {
        assert_eq!(surface_rgba(Theme::Light, false), light());
    }

    // UX1-2: the dark theme uses the dark `--color-surface` token.
    #[test]
    fn ux1_2_dark_theme_is_the_dark_surface() {
        assert_eq!(surface_rgba(Theme::Dark, false), dark());
    }

    // UX1-2: an explicit theme ignores the operating system.
    #[test]
    fn ux1_2_explicit_themes_ignore_the_system() {
        assert_eq!(surface_rgba(Theme::Light, true), light());
        assert_eq!(surface_rgba(Theme::Dark, true), dark());
    }

    // UX1-2: the system theme follows the operating system.
    #[test]
    fn ux1_2_system_theme_follows_the_system() {
        assert_eq!(surface_rgba(Theme::System, false), light());
        assert_eq!(surface_rgba(Theme::System, true), dark());
    }
}
