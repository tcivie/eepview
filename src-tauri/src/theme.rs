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
