// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Keyboard shortcuts of the contract, as menu items with accelerators. Pure table.

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

const BASE: [(&str, &str, &str, Menu); 25] = [
    ("new-tab", "New Tab", "CmdOrCtrl+T", Menu::File),
    ("close-tab", "Close Tab", "CmdOrCtrl+W", Menu::File),
    (
        "reopen-tab",
        "Reopen Closed Tab",
        "CmdOrCtrl+Shift+T",
        Menu::File,
    ),
    ("focus-address", "Open Location", "CmdOrCtrl+L", Menu::File),
    ("settings", "Settings", "CmdOrCtrl+Comma", Menu::File),
    ("open-find", "Find", "CmdOrCtrl+F", Menu::Edit),
    ("find-next", "Find Next", "CmdOrCtrl+G", Menu::Edit),
    (
        "find-prev",
        "Find Previous",
        "CmdOrCtrl+Shift+G",
        Menu::Edit,
    ),
    ("reload", "Reload", "CmdOrCtrl+R", Menu::View),
    (
        "hard-reload",
        "Reload Without Cache",
        "CmdOrCtrl+Shift+R",
        Menu::View,
    ),
    ("stop", "Stop", "Escape", Menu::View),
    ("zoom-in", "Zoom In", "CmdOrCtrl+Equal", Menu::View),
    ("zoom-out", "Zoom Out", "CmdOrCtrl+Minus", Menu::View),
    ("zoom-reset", "Actual Size", "CmdOrCtrl+Digit0", Menu::View),
    ("back", "Back", "CmdOrCtrl+BracketLeft", Menu::Go),
    ("forward", "Forward", "CmdOrCtrl+BracketRight", Menu::Go),
    ("back-alt", "Back", "Alt+ArrowLeft", Menu::Go),
    ("forward-alt", "Forward", "Alt+ArrowRight", Menu::Go),
    ("home", "Home", "CmdOrCtrl+Shift+H", Menu::Go),
    ("history", "Show History", "", Menu::Go),
    (
        "bookmark",
        "Bookmark This Page",
        "CmdOrCtrl+D",
        Menu::Bookmarks,
    ),
    (
        "bookmarks",
        "Show Bookmarks",
        "CmdOrCtrl+Shift+B",
        Menu::Bookmarks,
    ),
    ("next-tab", "Next Tab", "Ctrl+Tab", Menu::Window),
    ("prev-tab", "Previous Tab", "Ctrl+Shift+Tab", Menu::Window),
    ("tab-9", "Last Tab", "CmdOrCtrl+Digit9", Menu::Window),
];

/// Every shortcut. History is Cmd+Y on macOS and Ctrl+H elsewhere.
#[must_use]
pub fn table(mac: bool) -> Vec<Shortcut> {
    let mut list: Vec<Shortcut> = BASE
        .iter()
        .map(|(id, label, accel, menu)| item(id, label, accel, *menu))
        .collect();
    if let Some(history) = list.iter_mut().find(|s| s.id == "history") {
        history.accel = if mac { "CmdOrCtrl+Y" } else { "Ctrl+H" }.into();
    }
    for n in 1..=8 {
        let accel = format!("CmdOrCtrl+Digit{n}");
        list.push(item(
            &format!("tab-{n}"),
            &format!("Tab {n}"),
            &accel,
            Menu::Window,
        ));
    }
    list
}

fn item(id: &str, label: &str, accel: &str, menu: Menu) -> Shortcut {
    Shortcut {
        id: id.into(),
        label: label.into(),
        accel: accel.into(),
        menu,
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
        "new-tab" => Action::NewTab,
        "close-tab" => Action::CloseTab,
        "reopen-tab" => Action::ReopenTab,
        "next-tab" => Action::NextTab,
        "prev-tab" => Action::PrevTab,
        "focus-address" => Action::FocusAddress,
        "open-find" => Action::Find,
        "find-next" => Action::FindNext,
        "find-prev" => Action::FindPrev,
        "reload" => Action::Reload,
        "hard-reload" => Action::HardReload,
        "stop" => Action::Stop,
        _ => return None,
    })
}

fn page_action(id: &str) -> Option<Action> {
    Some(match id {
        "back" | "back-alt" => Action::Back,
        "forward" | "forward-alt" => Action::Forward,
        "home" => Action::Home,
        "bookmark" => Action::Bookmark,
        "bookmarks" => Action::Bookmarks,
        "history" => Action::History,
        "zoom-in" => Action::ZoomIn,
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
        assert_eq!(key(true), "CmdOrCtrl+Y");
        assert_eq!(key(false), "Ctrl+H");
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
