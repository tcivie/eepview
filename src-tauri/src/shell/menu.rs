// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The menu bar: every shortcut of the contract as a menu accelerator, except the bare Esc
//! of Stop (K4): Esc belongs to the focused field, and the page webviews report it
//! themselves (`shell::input`).

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Runtime};

use super::apply::with_core;
use super::input;
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
    eepview_platform::quiet_menus();
    let table = shortcuts::table(MAC);
    let menu = Menu::new(app)?;
    app_menu(app, &menu)?;
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

/// The macOS menu layout: an app menu first, and the macOS accelerators.
const MAC: bool = cfg!(target_os = "macos");

/// The accelerator of a shortcut, `None` when it has none. A bare Esc is never a menu
/// accelerator: a menu key comes before the focused field, so it would take Esc from the
/// address bar, the find field and the popups (K4).
fn accel(s: &Shortcut) -> Option<&str> {
    let menu_key = !s.accel.is_empty() && s.accel != ESCAPE;
    menu_key.then_some(s.accel.as_str())
}

/// The accelerator of Stop that only the page webviews handle.
const ESCAPE: &str = "Escape";

/// True for the Stop item, which starts disabled and follows the active tab's loading.
fn is_stop(s: &Shortcut) -> bool {
    s.id == "stop"
}

fn menu_item<R: Runtime>(app: &AppHandle<R>, s: &Shortcut) -> tauri::Result<MenuItem<R>> {
    let item = MenuItem::with_id(app, s.id.as_str(), &s.label, !is_stop(s), accel(s))?;
    if is_stop(s) {
        *lock(&shared(app).stop_item) = Some(item.clone());
    }
    Ok(item)
}

/// The macOS app menu (About, Hide, Quit); other systems have none.
fn app_menu<R: Runtime>(app: &AppHandle<R>, menu: &Menu<R>) -> tauri::Result<()> {
    for sub in app_submenus(app) {
        menu.append(&sub?)?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn app_submenus<R: Runtime>(app: &AppHandle<R>) -> Vec<tauri::Result<Submenu<R>>> {
    vec![app_submenu(app)]
}

#[cfg(not(target_os = "macos"))]
fn app_submenus<R: Runtime>(_app: &AppHandle<R>) -> Vec<tauri::Result<Submenu<R>>> {
    Vec::new()
}

#[cfg(target_os = "macos")]
fn app_submenu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
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

/// A menu item was chosen: a context menu item, or a menu bar item.
pub fn on_event<R: Runtime>(app: &AppHandle<R>, id: &str) {
    if id.starts_with(input::MENU_PREFIX) {
        input::chosen(app, id);
    } else if let Some(action) = shortcuts::action(id) {
        with_core(app, |core| core.shortcut(action, now_ms()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::testing::{bare, core};

    #[test]
    fn only_stop_starts_disabled() {
        let table = shortcuts::table(MAC);
        let stops: Vec<&str> = table
            .iter()
            .filter(|s| is_stop(s))
            .map(|s| s.id.as_str())
            .collect();
        assert_eq!(stops, ["stop"]);
    }

    #[test]
    fn empty_accelerators_are_none() {
        let mut item = shortcuts::table(MAC)[0].clone();
        assert_eq!(
            accel(&item),
            Some(item.accel.as_str()).filter(|a| !a.is_empty())
        );
        item.accel.clear();
        assert_eq!(accel(&item), None);
    }

    #[test]
    fn every_group_has_items() {
        let table = shortcuts::table(MAC);
        for (group, title) in GROUPS {
            assert!(table.iter().any(|s| s.menu == group), "{title}");
        }
    }

    #[test]
    fn menu_events_run_the_shortcut() {
        let app = bare();
        on_event(app.handle(), "new-tab");
        assert_eq!(core(&app).tabs().len(), 2);
        on_event(app.handle(), "no-such-item");
        assert_eq!(core(&app).tabs().len(), 2);
    }

    /// muda builds menus only on the main thread on macOS, and tests run on other threads.
    #[cfg(target_os = "linux")]
    #[test]
    fn the_menu_bar_builds() {
        let app = bare();
        let menu = build(app.handle()).unwrap();
        assert_eq!(menu.items().unwrap().len(), GROUPS.len());
        assert!(lock(&shared(app.handle()).stop_item).is_some());
        crate::shell::view::sync_stop_item(app.handle());
    }
}
