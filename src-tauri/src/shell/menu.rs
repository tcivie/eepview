// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The menu bar: every shortcut of the contract as a menu accelerator.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Runtime};

use super::apply::with_core;
use super::state::{lock, now_ms, shared};
use crate::shortcuts::{self, Menu as Group, Shortcut};

const GROUPS: [(Group, &str); 6] = [
    (Group::File, "File"),
    (Group::Edit, "Edit"),
    (Group::View, "View"),
    (Group::Go, "History"),
    (Group::Bookmarks, "Bookmarks"),
    (Group::Window, "Window"),
];

/// Builds the menu bar.
///
/// # Errors
///
/// Fails when the OS menu cannot be built.
pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let table = shortcuts::table(cfg!(target_os = "macos"));
    let menu = Menu::new(app)?;
    if cfg!(target_os = "macos") {
        menu.append(&app_menu(app)?)?;
    }
    for (group, title) in GROUPS {
        menu.append(&group_menu(app, &table, group, title)?)?;
    }
    Ok(menu)
}

fn group_menu<R: Runtime>(
    app: &AppHandle<R>,
    table: &[Shortcut],
    group: Group,
    title: &str,
) -> tauri::Result<Submenu<R>> {
    let sub = Submenu::new(app, title, true)?;
    if group == Group::Edit {
        edit_items(app, &sub)?;
    }
    for item in table.iter().filter(|s| s.menu == group) {
        sub.append(&menu_item(app, item)?)?;
    }
    Ok(sub)
}

fn menu_item<R: Runtime>(app: &AppHandle<R>, s: &Shortcut) -> tauri::Result<MenuItem<R>> {
    let accel = (!s.accel.is_empty()).then_some(s.accel.as_str());
    let enabled = s.id != "stop";
    let item = MenuItem::with_id(app, s.id.as_str(), &s.label, enabled, accel)?;
    if s.id == "stop" {
        *lock(&shared(app).stop_item) = Some(item.clone());
    }
    Ok(item)
}

fn app_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    let sub = Submenu::new(app, "eepview", true)?;
    let items = [
        PredefinedMenuItem::about(app, None, None),
        PredefinedMenuItem::separator(app),
        PredefinedMenuItem::hide(app, None),
        PredefinedMenuItem::hide_others(app, None),
        PredefinedMenuItem::separator(app),
        PredefinedMenuItem::quit(app, None),
    ];
    append_all(&sub, items)?;
    Ok(sub)
}

/// Undo, cut, copy, paste and select all: without them the address bar has no clipboard
/// keys on macOS.
fn edit_items<R: Runtime>(app: &AppHandle<R>, sub: &Submenu<R>) -> tauri::Result<()> {
    let items = [
        PredefinedMenuItem::undo(app, None),
        PredefinedMenuItem::redo(app, None),
        PredefinedMenuItem::separator(app),
        PredefinedMenuItem::cut(app, None),
        PredefinedMenuItem::copy(app, None),
        PredefinedMenuItem::paste(app, None),
        PredefinedMenuItem::select_all(app, None),
        PredefinedMenuItem::separator(app),
    ];
    append_all(sub, items)
}

fn append_all<R: Runtime, const N: usize>(
    sub: &Submenu<R>,
    items: [tauri::Result<PredefinedMenuItem<R>>; N],
) -> tauri::Result<()> {
    for item in items {
        sub.append(&item?)?;
    }
    Ok(())
}

/// A menu item was chosen.
pub fn on_event<R: Runtime>(app: &AppHandle<R>, id: &str) {
    if let Some(action) = shortcuts::action(id) {
        with_core(app, |core| core.shortcut(action, now_ms()));
    }
}
