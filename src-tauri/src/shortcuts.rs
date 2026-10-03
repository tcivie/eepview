// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Keyboard shortcuts of the contract (docs/wiki/links-and-shortcuts.md, K1), as menu items
//! with accelerators and as a chord lookup for keys the menu bar cannot see. Pure table.

use crate::input::{Modifiers, MouseButton};

/// What a shortcut does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// New tab.
    NewTab,
    /// Close the active tab.
    CloseTab,
    /// Reopen the last closed tab.
    ReopenTab,
    /// Next tab.
    NextTab,
    /// Previous tab.
    PrevTab,
    /// Tab n (1–8), or the last tab (9).
    SelectTab(usize),
    /// Focus the address bar (the UI does it).
    FocusAddress,
    /// Open the find bar (the UI does it).
    Find,
    /// Next match.
    FindNext,
    /// Previous match.
    FindPrev,
    /// Reload.
    Reload,
    /// Reload without the cache.
    HardReload,
    /// Back.
    Back,
    /// Forward.
    Forward,
    /// Homepage.
    Home,
    /// Bookmark the active page.
    Bookmark,
    /// Bookmarks page.
    Bookmarks,
    /// History page.
    History,
    /// Zoom in.
    ZoomIn,
    /// Zoom out.
    ZoomOut,
    /// Zoom 100 %.
    ZoomReset,
    /// Stop loading.
    Stop,
    /// Settings page.
    Settings,
}

/// The menu a shortcut lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    /// File.
    File,
    /// Edit (find).
    Edit,
    /// View (reload, zoom).
    View,
    /// History (back, forward, home).
    Go,
    /// Bookmarks.
    Bookmarks,
    /// Window (tabs).
    Window,
}

/// One menu item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shortcut {
    /// Menu item id; also the `shortcut` event action for UI actions.
    pub id: String,
    /// Menu label.
    pub label: String,
    /// Accelerator in Tauri syntax.
    pub accel: String,
    /// The menu that holds it.
    pub menu: Menu,
}

/// The rows of K1 that every system has.
const COMMON: [(&str, &str, &str, Menu); 32] = [
    ("new-tab", "New Tab", "CmdOrCtrl+KeyT", Menu::File),
    ("new-window", "New Window", "CmdOrCtrl+KeyN", Menu::File),
    ("close-tab", "Close Tab", "CmdOrCtrl+KeyW", Menu::File),
    (
        "reopen-tab",
        "Reopen Closed Tab",
        "CmdOrCtrl+Shift+KeyT",
        Menu::File,
    ),
    (
        "focus-address",
        "Open Location",
        "CmdOrCtrl+KeyL",
        Menu::File,
    ),
    ("settings", "Settings", "CmdOrCtrl+Comma", Menu::File),
    ("open-find", "Find", "CmdOrCtrl+KeyF", Menu::Edit),
    ("find-next", "Find Next", "CmdOrCtrl+KeyG", Menu::Edit),
    (
        "find-prev",
        "Find Previous",
        "CmdOrCtrl+Shift+KeyG",
        Menu::Edit,
    ),
    ("reload", "Reload", "CmdOrCtrl+KeyR", Menu::View),
    (
        "hard-reload",
        "Reload Without Cache",
        "CmdOrCtrl+Shift+KeyR",
        Menu::View,
    ),
    ("stop", "Stop", "Escape", Menu::View),
    ("zoom-in", "Zoom In", "CmdOrCtrl+Equal", Menu::View),
    (
        "zoom-in-plus",
        "Zoom In",
        "CmdOrCtrl+Shift+Equal",
        Menu::View,
    ),
    ("zoom-out", "Zoom Out", "CmdOrCtrl+Minus", Menu::View),
    ("zoom-reset", "Actual Size", "CmdOrCtrl+Digit0", Menu::View),
    ("back", "Back", "CmdOrCtrl+BracketLeft", Menu::Go),
    ("forward", "Forward", "CmdOrCtrl+BracketRight", Menu::Go),
    ("home", "Home", "CmdOrCtrl+Shift+KeyH", Menu::Go),
    (
        "bookmark",
        "Bookmark This Page",
        "CmdOrCtrl+KeyD",
        Menu::Bookmarks,
    ),
    (
        "bookmarks",
        "Show Bookmarks",
        "CmdOrCtrl+Shift+KeyB",
        Menu::Bookmarks,
    ),
    ("next-tab", "Next Tab", "Ctrl+Tab", Menu::Window),
    ("prev-tab", "Previous Tab", "Ctrl+Shift+Tab", Menu::Window),
    ("tab-1", "Tab 1", "CmdOrCtrl+Digit1", Menu::Window),
    ("tab-2", "Tab 2", "CmdOrCtrl+Digit2", Menu::Window),
    ("tab-3", "Tab 3", "CmdOrCtrl+Digit3", Menu::Window),
    ("tab-4", "Tab 4", "CmdOrCtrl+Digit4", Menu::Window),
    ("tab-5", "Tab 5", "CmdOrCtrl+Digit5", Menu::Window),
    ("tab-6", "Tab 6", "CmdOrCtrl+Digit6", Menu::Window),
    ("tab-7", "Tab 7", "CmdOrCtrl+Digit7", Menu::Window),
    ("tab-8", "Tab 8", "CmdOrCtrl+Digit8", Menu::Window),
    ("tab-9", "Last Tab", "CmdOrCtrl+Digit9", Menu::Window),
];

/// The rows of K1 on macOS only.
const MAC_ONLY: [(&str, &str, &str, Menu); 4] = [
    ("stop-period", "Stop", "CmdOrCtrl+Period", Menu::View),
    ("history", "Show History", "CmdOrCtrl+KeyY", Menu::Go),
    (
        "next-tab-alt",
        "Next Tab",
        "CmdOrCtrl+Shift+BracketRight",
        Menu::Window,
    ),
    (
        "prev-tab-alt",
        "Previous Tab",
        "CmdOrCtrl+Shift+BracketLeft",
        Menu::Window,
    ),
];

/// The rows of K1 on Windows and Linux only.
const OTHER_ONLY: [(&str, &str, &str, Menu); 11] = [
    ("close-tab-f4", "Close Tab", "Ctrl+F4", Menu::File),
    ("focus-address-alt", "Open Location", "Alt+KeyD", Menu::File),
    ("focus-address-f6", "Open Location", "F6", Menu::File),
    ("reload-f5", "Reload", "F5", Menu::View),
    (
        "hard-reload-f5",
        "Reload Without Cache",
        "Ctrl+F5",
        Menu::View,
    ),
    ("back-alt", "Back", "Alt+ArrowLeft", Menu::Go),
    ("forward-alt", "Forward", "Alt+ArrowRight", Menu::Go),
    ("home-alt", "Home", "Alt+Home", Menu::Go),
    ("history", "Show History", "Ctrl+KeyH", Menu::Go),
    ("next-tab-alt", "Next Tab", "Ctrl+PageDown", Menu::Window),
    ("prev-tab-alt", "Previous Tab", "Ctrl+PageUp", Menu::Window),
];

/// Every shortcut of K1 for one system (`mac` true for macOS). Quit is the macOS app menu
/// item, not a row.
#[must_use]
pub fn table(mac: bool) -> Vec<Shortcut> {
    let own: &[(&str, &str, &str, Menu)] = if mac { &MAC_ONLY } else { &OTHER_ONLY };
    COMMON
        .iter()
        .chain(own)
        .map(|(id, label, accel, menu)| Shortcut {
            id: (*id).into(),
            label: (*label).into(),
            accel: (*accel).into(),
            menu: *menu,
        })
        .collect()
}

/// One key press: a `KeyboardEvent.code` name and the held modifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Chord {
    /// The key, as a `KeyboardEvent.code` name (`KeyT`, `Digit1`, `Escape`, `F5`).
    pub code: String,
    /// The held modifiers.
    pub modifiers: Modifiers,
}

/// The chord of an accelerator such as `CmdOrCtrl+Shift+KeyT`; `CmdOrCtrl` is Cmd on macOS
/// and Ctrl elsewhere.
fn chord_of(mac: bool, accel: &str) -> Chord {
    let mut parts: Vec<&str> = accel.split('+').collect();
    let code = parts.pop().unwrap_or_default().to_owned();
    let mut modifiers = Modifiers::default();
    for part in parts {
        match part {
            "CmdOrCtrl" if mac => modifiers.meta = true,
            "Alt" => modifiers.alt = true,
            "Shift" => modifiers.shift = true,
            _ => modifiers.ctrl = true,
        }
    }
    Chord { code, modifiers }
}

/// The action of the K1 row whose keys are exactly `chord` on this system (K1, K9).
#[must_use]
pub fn lookup(mac: bool, chord: &Chord) -> Option<Action> {
    table(mac)
        .into_iter()
        .find(|s| chord_of(mac, &s.accel) == *chord)
        .and_then(|s| action(&s.id))
}

/// The action of a mouse button: back and forward only (B3).
#[must_use]
pub fn mouse_action(button: MouseButton) -> Option<Action> {
    match button {
        MouseButton::Back => Some(Action::Back),
        MouseButton::Forward => Some(Action::Forward),
        _ => None,
    }
}

/// The action of a menu item id.
#[must_use]
pub fn action(id: &str) -> Option<Action> {
    if let Some(n) = id.strip_prefix("tab-") {
        return n
            .parse()
            .ok()
            .filter(|n| (1..=9).contains(n))
            .map(Action::SelectTab);
    }
    simple_action(id).or_else(|| page_action(id))
}

fn simple_action(id: &str) -> Option<Action> {
    Some(match id {
        "new-tab" | "new-window" => Action::NewTab,
        "close-tab" | "close-tab-f4" => Action::CloseTab,
        "reopen-tab" => Action::ReopenTab,
        "next-tab" | "next-tab-alt" => Action::NextTab,
        "prev-tab" | "prev-tab-alt" => Action::PrevTab,
        "focus-address" | "focus-address-alt" | "focus-address-f6" => Action::FocusAddress,
        "open-find" => Action::Find,
        "find-next" => Action::FindNext,
        "find-prev" => Action::FindPrev,
        "reload" | "reload-f5" => Action::Reload,
        "hard-reload" | "hard-reload-f5" => Action::HardReload,
        "stop" | "stop-period" => Action::Stop,
        _ => return None,
    })
}

fn page_action(id: &str) -> Option<Action> {
    Some(match id {
        "back" | "back-alt" => Action::Back,
        "forward" | "forward-alt" => Action::Forward,
        "home" | "home-alt" => Action::Home,
        "bookmark" => Action::Bookmark,
        "bookmarks" => Action::Bookmarks,
        "history" => Action::History,
        "zoom-in" | "zoom-in-plus" => Action::ZoomIn,
        "zoom-out" => Action::ZoomOut,
        "zoom-reset" => Action::ZoomReset,
        "settings" => Action::Settings,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_item_has_an_action_and_unique_keys() {
        for mac in [true, false] {
            let list = table(mac);
            let ids: HashSet<&str> = list.iter().map(|s| s.id.as_str()).collect();
            let keys: HashSet<&str> = list.iter().map(|s| s.accel.as_str()).collect();
            assert_eq!(ids.len(), list.len());
            assert_eq!(keys.len(), list.len());
            assert!(list.iter().all(|s| action(&s.id).is_some()));
        }
    }

    #[test]
    fn history_key_per_platform() {
        let key = |mac| {
            table(mac)
                .into_iter()
                .find(|s| s.id == "history")
                .unwrap()
                .accel
        };
        assert_eq!(key(true), "CmdOrCtrl+KeyY");
        assert_eq!(key(false), "Ctrl+KeyH");
    }

    #[test]
    fn tab_numbers() {
        assert_eq!(action("tab-1"), Some(Action::SelectTab(1)));
        assert_eq!(action("tab-9"), Some(Action::SelectTab(9)));
        assert_eq!(action("tab-0"), None);
        assert_eq!(action("tab-x"), None);
        assert_eq!(action("nope"), None);
        assert_eq!(action("back-alt"), Some(Action::Back));
    }
}
