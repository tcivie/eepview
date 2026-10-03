// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The surface colour of the active theme, read from the design tokens in `theme.css`.

use crate::store::settings::Theme;

const CSS: &str = include_str!("../../src/ui/theme.css");
/// The rule that holds the light tokens, and the rule that holds the dark ones.
const LIGHT_RULE: &str = ":root {";
const DARK_RULE: &str = ":root[data-theme=\"dark\"] {";

/// True when the theme shows dark surfaces. `system_dark` says whether the operating system
/// is in dark mode; it counts only for [`Theme::System`].
#[must_use]
pub fn is_dark(theme: Theme, system_dark: bool) -> bool {
    match theme {
        Theme::Dark => true,
        Theme::Light => false,
        Theme::System => system_dark,
    }
}

/// The `--color-surface` token of the theme as `[red, green, blue, alpha]`, read from
/// `theme.css`, the one place that holds the palette.
#[must_use]
pub fn surface_rgba(theme: Theme, system_dark: bool) -> [u8; 4] {
    if is_dark(theme, system_dark) {
        surface_in(CSS, DARK_RULE).unwrap_or([0, 0, 0, 255])
    } else {
        surface_in(CSS, LIGHT_RULE).unwrap_or([255; 4])
    }
}

/// The `--color-surface` value in the first `rule` of `css`.
fn surface_in(css: &str, rule: &str) -> Option<[u8; 4]> {
    let from = css.find(rule)?;
    let value = css[from..]
        .split_once("--color-surface:")?
        .1
        .split_once(';')?
        .0;
    parse_hex(value.trim())
}

/// `#rrggbb` as an opaque colour.
fn parse_hex(value: &str) -> Option<[u8; 4]> {
    let hex = value.strip_prefix('#').filter(|h| h.len() == 6)?;
    let [_, red, green, blue] = u32::from_str_radix(hex, 16).ok()?.to_be_bytes();
    Some([red, green, blue, 255])
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSS: &str = include_str!("../../src/ui/theme.css");

    #[test]
    fn a_missing_token_falls_back_to_plain_white_or_black() {
        assert_eq!(surface_in("", DARK_RULE), None);
        assert_eq!(surface_in(":root {}", LIGHT_RULE), None);
        assert!(is_dark(Theme::System, true));
        assert!(!is_dark(Theme::System, false));
    }

    #[test]
    fn hex_values_parse_and_bad_ones_do_not() {
        assert_eq!(parse_hex("#0a0b0c"), Some([10, 11, 12, 255]));
        assert_eq!(parse_hex("#abc"), None);
        assert_eq!(parse_hex("red"), None);
        assert_eq!(parse_hex("#gggggg"), None);
        assert_eq!(surface_in("a { b: c }", ":root {"), None);
    }

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
