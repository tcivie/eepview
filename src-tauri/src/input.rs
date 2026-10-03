// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Where a link opens: the held keys and the mouse button decide (docs/wiki/links-and-shortcuts.md,
//! L1 to L7). Pure.

/// The modifier keys held during a click or a key press.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    /// Command on macOS, the Windows (Super) key elsewhere.
    pub meta: bool,
    /// Control.
    pub ctrl: bool,
    /// Alt (Option on macOS).
    pub alt: bool,
    /// Shift.
    pub shift: bool,
}

/// The mouse button of a click.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    /// A keyboard activation, such as Enter on a link.
    None,
    /// The primary (left) button.
    Primary,
    /// The middle button or wheel.
    Middle,
    /// The secondary (right) button.
    Secondary,
    /// The back button.
    Back,
    /// The forward button.
    Forward,
}

/// Where a link opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Disposition {
    /// In the tab that shows the link.
    CurrentTab,
    /// In a new tab that does not become active.
    NewBackgroundTab,
    /// In a new tab that becomes active.
    NewForegroundTab,
}

/// Where a link click opens (L1 to L7). `mac` picks the new-tab key: Cmd on macOS, Ctrl
/// elsewhere. On macOS a Ctrl click is the context menu, so it never opens a tab.
#[must_use]
pub fn link_disposition(mac: bool, modifiers: Modifiers, button: MouseButton) -> Disposition {
    let new_tab_key = if mac { modifiers.meta } else { modifiers.ctrl };
    let opens_tab = match button {
        MouseButton::Middle => true,
        MouseButton::Primary | MouseButton::None if mac && modifiers.ctrl => {
            return Disposition::CurrentTab;
        }
        MouseButton::Primary | MouseButton::None => new_tab_key,
        MouseButton::Secondary | MouseButton::Back | MouseButton::Forward => {
            return Disposition::CurrentTab;
        }
    };
    if modifiers.shift {
        Disposition::NewForegroundTab
    } else if opens_tab {
        Disposition::NewBackgroundTab
    } else {
        Disposition::CurrentTab
    }
}

/// True on macOS: Cmd is the new-tab key and Ctrl+click is the context menu.
pub const MAC: bool = cfg!(target_os = "macos");
