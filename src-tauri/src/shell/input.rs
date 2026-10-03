// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The input that the engines report through `eepview-platform` (docs/wiki/links-and-shortcuts.md):
//! link clicks that open a tab, keys the menu bar cannot see, the mouse back and forward
//! buttons, and context menu requests. The decisions are pure functions ([`key_action`],
//! [`link_action`], [`menu_target`], [`menu_entries`], [`menu_act`]); [`handle`] and
//! [`chosen`] carry them out.
//!
//! Security: a link or a menu item opens a tab only through [`Core::open_link`], which
//! refuses anything but an I2P URL (L9). A link of the router console stays in the console
//! view or opens a normal tab through the same guard. No page gets IPC from here.

use eepview_platform::{Hit, Hooks, Input, MenuEntry, Native, PlatformWebview, Reply};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::{AppHandle, Manager, Runtime, Url};

use super::apply::{outside, with_core};
use super::state::{lock, now_ms, shared};
use crate::context_menu::{Entry, ItemId, PageHistory, Target, context_menu};
use crate::core::Core;
use crate::diag::{self, Code, ErrorKind, Field, OpKind};
use crate::input::{Disposition, MAC, Modifiers, MouseButton, link_disposition};
use crate::shortcuts::{self, Action, Chord};

/// The prefix of a context menu item id, apart from the menu bar ids.
pub const MENU_PREFIX: &str = "ctx:";

/// The webview that reported an input event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The `tab-*` webview of a tab.
    Tab(u32),
    /// The `internal` webview (Home, Bookmarks, History, Settings).
    Internal,
    /// The `console` webview of the router console tab.
    Console,
    /// A chrome webview: `toolbar`, `status` or `popup`.
    Chrome(&'static str),
}

impl Source {
    /// True for a page webview: Esc there stops a load (K4), and its links can open tabs.
    #[must_use]
    pub fn content(self) -> bool {
        !matches!(self, Self::Chrome(_))
    }

    /// The webview label, for a tab only while its webview lives.
    fn label<R: Runtime>(self, app: &AppHandle<R>) -> Option<String> {
        match self {
            Self::Tab(tab) => lock(&shared(app).labels).get(&tab).cloned(),
            Self::Internal => Some("internal".into()),
            Self::Console => Some(super::console::CONSOLE_LABEL.into()),
            Self::Chrome(label) => Some(label.into()),
        }
    }
}

/// What a context menu item does (pure result of [`menu_act`]).
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    /// Nothing.
    None,
    /// Open a URL in a new tab, through [`Core::open_link`].
    Open(Url, Disposition),
    /// Put this text on the clipboard.
    Copy(String),
    /// Run a shortcut action on the active tab.
    Shortcut(Action),
    /// An editing command of the engine.
    Edit(Native),
}

/// The action of a key press from `source` (K1, K4, K9). Esc stops a load only from a page
/// webview: in the address bar, the find field and the popups it belongs to the field.
#[must_use]
pub fn key_action(mac: bool, source: Source, code: &str, keys: Modifiers) -> Option<Action> {
    if code == "Escape" && !source.content() {
        return None;
    }
    let chord = Chord {
        code: code.to_owned(),
        modifiers: keys,
    };
    shortcuts::lookup(mac, &chord)
}

/// Where a reported link click opens, or `None` when the shell must not act: a plain click
/// (the engine follows it), a Ctrl+click on macOS (the context menu), and the secondary,
/// back and forward buttons (L1 to L7, C1, B3).
#[must_use]
pub fn link_action(mac: bool, keys: Modifiers, button: MouseButton) -> Option<Disposition> {
    if !matches!(
        button,
        MouseButton::None | MouseButton::Primary | MouseButton::Middle
    ) {
        return None;
    }
    match link_disposition(mac, keys, button) {
        Disposition::CurrentTab => None,
        how => Some(how),
    }
}

/// Where a page's own new-window request opens: held new-tab keys follow L2 to L5; with no
/// such key, `None` keeps the plain foreground tab of [`Core::new_window`] (L12).
#[must_use]
pub fn window_action(mac: bool, keys: Modifiers) -> Option<Disposition> {
    link_action(mac, keys, MouseButton::Primary)
}

/// The menu target of a request from `source`, or `None` for no menu: a chrome webview
/// outside a text field shows none (C12). Page facts come from the tab of `source`.
#[must_use]
pub fn menu_target(core: &Core, source: Source, hit: Hit) -> Option<Target> {
    let tab = match source {
        Source::Chrome(_) if !hit.editable => return None,
        Source::Chrome(_) => {
            return Some(Target {
                selection: hit.selection,
                editable: true,
                ..Target::default()
            });
        }
        Source::Tab(id) => Some(id),
        Source::Internal => Some(core.tabs().active_id()),
        Source::Console => core.console_tab(),
    };
    let info = tab.and_then(|id| core.tab_info(id));
    let (page, history, bookmarked) = info.map_or_else(Default::default, |i| {
        let history = PageHistory {
            can_back: i.nav.can_back,
            can_forward: i.nav.can_forward,
        };
        (i.url, history, i.marks.bookmarked)
    });
    Some(Target {
        link: hit.link,
        image: hit.image,
        selection: hit.selection,
        editable: hit.editable,
        page,
        history,
        bookmarked,
    })
}

/// The engine command behind an editing item.
#[must_use]
pub fn native_of(item: ItemId) -> Option<Native> {
    Some(match item {
        ItemId::Undo => Native::Undo,
        ItemId::Redo => Native::Redo,
        ItemId::Cut => Native::Cut,
        ItemId::Copy => Native::Copy,
        ItemId::Paste => Native::Paste,
        ItemId::SelectAll => Native::SelectAll,
        ItemId::CopyImage => Native::CopyImage,
        _ => return None,
    })
}

/// The bridge entries of a model menu: ids get [`MENU_PREFIX`], labels come from the model.
#[must_use]
pub fn menu_entries(menu: &[Entry]) -> Vec<MenuEntry> {
    menu.iter()
        .map(|entry| match *entry {
            Entry::Item { id, enabled } => MenuEntry::Item {
                id: format!("{MENU_PREFIX}{}", id.as_str()),
                label: id.label().to_owned(),
                enabled,
                native: native_of(id),
            },
            Entry::Separator => MenuEntry::Separator,
        })
        .collect()
}

/// The item of a context menu id (with [`MENU_PREFIX`]).
#[must_use]
pub fn menu_item_of(id: &str) -> Option<ItemId> {
    id.strip_prefix(MENU_PREFIX).and_then(ItemId::parse)
}

fn open_act(url: Option<&String>, how: Disposition) -> Act {
    url.and_then(|u| Url::parse(u).ok())
        .map_or(Act::None, |u| Act::Open(u, how))
}

fn copy_act(text: Option<&String>) -> Act {
    text.map_or(Act::None, |t| Act::Copy(t.clone()))
}

/// What a chosen context menu item does with its target (C9 to C11).
#[must_use]
pub fn menu_act(item: ItemId, target: &Target) -> Act {
    match item {
        ItemId::OpenLinkInNewTab => open_act(target.link.as_ref(), Disposition::NewForegroundTab),
        ItemId::OpenLinkInBackgroundTab => {
            open_act(target.link.as_ref(), Disposition::NewBackgroundTab)
        }
        ItemId::OpenImageInNewTab => open_act(target.image.as_ref(), Disposition::NewForegroundTab),
        ItemId::CopyLinkAddress => copy_act(target.link.as_ref()),
        ItemId::CopyImageAddress => copy_act(target.image.as_ref()),
        ItemId::CopyPageAddress => Act::Copy(target.page.clone()),
        other => page_act(other),
    }
}

fn page_act(item: ItemId) -> Act {
    match item {
        ItemId::Back => Act::Shortcut(Action::Back),
        ItemId::Forward => Act::Shortcut(Action::Forward),
        ItemId::Reload => Act::Shortcut(Action::Reload),
        ItemId::BookmarkPage => Act::Shortcut(Action::Bookmark),
        ItemId::Find => Act::Shortcut(Action::Find),
        other => native_of(other).map_or(Act::None, Act::Edit),
    }
}

/// The hooks of one webview for [`eepview_platform::on_input`].
pub fn hooks<R: Runtime>(app: &AppHandle<R>, source: Source) -> Hooks {
    let (input_app, chosen_app) = (app.clone(), app.clone());
    Hooks {
        content: source.content(),
        input: Box::new(move |input| handle(&input_app, source, input)),
        chosen: Box::new(move |id| chosen(&chosen_app, id)),
    }
}

/// Installs the input hooks on a webview. Call it inside `with_webview`.
pub fn install<R: Runtime>(app: &AppHandle<R>, platform: &PlatformWebview, source: Source) {
    if eepview_platform::on_input(platform, hooks(app, source)).is_err() {
        diag::event(
            Code::EngineCallFailed,
            &[Field::Op(OpKind::Input), Field::Error(ErrorKind::Platform)],
        );
    }
}

fn run_shortcut<R: Runtime>(app: &AppHandle<R>, action: Action) {
    with_core(app, |core| core.shortcut(action, now_ms()));
}

/// Acts on one input event of `source` and tells the engine whether it may go on.
pub fn handle<R: Runtime>(app: &AppHandle<R>, source: Source, input: Input) -> Reply {
    match input {
        Input::Key { code, keys } => match key_action(MAC, source, &code, keys) {
            // Esc stops the load, and the page still gets it (K4).
            Some(action) => {
                run_shortcut(app, action);
                if code == "Escape" {
                    Reply::Pass
                } else {
                    Reply::Consume
                }
            }
            None => Reply::Pass,
        },
        Input::Mouse(button) => shortcuts::mouse_action(button).map_or(Reply::Pass, |action| {
            run_shortcut(app, action);
            Reply::Consume
        }),
        Input::Link { url, keys, button } => {
            let target = Url::parse(&url).ok();
            match (target, link_action(MAC, keys, button)) {
                (Some(url), Some(how)) => {
                    open_link(app, source, &url, how);
                    Reply::Consume
                }
                _ => Reply::Pass,
            }
        }
        Input::Menu(hit) => menu_reply(app, source, hit),
    }
}

/// A page asked for a new window (`target=_blank`, `window.open`, or a modified click that
/// the engine turned into one): never an engine window. Held new-tab keys follow L2 to L5;
/// with none, a foreground tab (L12). An internal page link stays in the internal webview.
pub fn new_window<R: Runtime>(app: &AppHandle<R>, source: Source, url: &Url) {
    match window_action(MAC, eepview_platform::held_keys()) {
        Some(how) => open_link(app, source, url, how),
        None if source == Source::Internal && super::chrome::bundled(app, url) => {
            load_internal(app, url);
        }
        None => with_core(app, |core| core.new_window(url)),
    }
}

/// Installs the input hooks on the bundled webviews.
pub fn wire_chrome<R: Runtime>(app: &AppHandle<R>) {
    let sources = [
        ("toolbar", Source::Chrome("toolbar")),
        ("internal", Source::Internal),
        ("status", Source::Chrome("status")),
        ("popup", Source::Chrome("popup")),
    ];
    for (label, source) in sources {
        if let Some(webview) = app.get_webview(label) {
            let handle = app.clone();
            let _ = webview.with_webview(move |p| install(&handle, &p, source));
        }
    }
}

/// A link that opens a tab. An internal page link stays a plain click in the internal
/// webview; a console link stays in the console view or opens a normal I2P tab.
fn open_link<R: Runtime>(app: &AppHandle<R>, source: Source, url: &Url, how: Disposition) {
    match source {
        Source::Console => super::console::open_link(app, url, how),
        Source::Internal if super::chrome::bundled(app, url) => load_internal(app, url),
        _ => with_core(app, |core| core.open_link(url, how)),
    }
}

fn load_internal<R: Runtime>(app: &AppHandle<R>, url: &Url) {
    if let Some(webview) = app.get_webview("internal") {
        let url = url.clone();
        outside(move || {
            let _ = webview.navigate(url);
        });
    }
}

/// The reply to a menu request: the target is kept for [`chosen`]. Windows and Linux get
/// the entries for the engine menu; macOS shows its own menu at the pointer.
fn menu_reply<R: Runtime>(app: &AppHandle<R>, source: Source, hit: Hit) -> Reply {
    let target = menu_target(&lock(&shared(app).core), source, hit);
    let entries = target
        .as_ref()
        .map(|t| menu_entries(&context_menu(t)))
        .unwrap_or_default();
    let label = source.label(app).unwrap_or_default();
    *lock(&shared(app).menu_target) = target.map(|t| (label, t));
    if MAC {
        show_menu(app, entries);
        return Reply::Consume;
    }
    Reply::Menu(entries)
}

/// Shows the menu at the pointer, off the engine callback (macOS).
fn show_menu<R: Runtime>(app: &AppHandle<R>, entries: Vec<MenuEntry>) {
    if entries.is_empty() {
        return;
    }
    let handle = app.clone();
    outside(move || {
        let runner = handle.clone();
        let _ = runner.run_on_main_thread(move || popup(&handle, &entries));
    });
}

fn popup<R: Runtime>(app: &AppHandle<R>, entries: &[MenuEntry]) {
    let Some(window) = app.get_window("main") else {
        return;
    };
    if let Ok(menu) = build_menu(app, entries) {
        let _ = window.popup_menu(&menu);
    }
}

fn build_menu<R: Runtime>(app: &AppHandle<R>, entries: &[MenuEntry]) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(app)?;
    for entry in entries {
        match entry {
            MenuEntry::Item {
                id, label, enabled, ..
            } => menu.append(&MenuItem::with_id(
                app,
                id.as_str(),
                label,
                *enabled,
                None::<&str>,
            )?)?,
            MenuEntry::Separator => menu.append(&PredefinedMenuItem::separator(app)?)?,
        }
    }
    Ok(menu)
}

/// A context menu item was chosen: it acts on the target of the last request.
pub fn chosen<R: Runtime>(app: &AppHandle<R>, id: &str) {
    let Some(item) = menu_item_of(id) else {
        return;
    };
    let Some((label, target)) = lock(&shared(app).menu_target).clone() else {
        return;
    };
    run_act(app, &label, menu_act(item, &target));
}

fn run_act<R: Runtime>(app: &AppHandle<R>, label: &str, act: Act) {
    match act {
        Act::None => {}
        Act::Open(url, how) => with_core(app, |core| core.open_link(&url, how)),
        Act::Copy(text) => copy(&text),
        Act::Shortcut(action) => run_shortcut(app, action),
        Act::Edit(native) => edit(app, label, native),
    }
}

fn copy(text: &str) {
    if eepview_platform::copy_text(text).is_err() {
        diag::event(
            Code::EngineCallFailed,
            &[
                Field::Op(OpKind::Clipboard),
                Field::Error(ErrorKind::Platform),
            ],
        );
    }
}

fn edit<R: Runtime>(app: &AppHandle<R>, label: &str, native: Native) {
    if let Some(webview) = app.get_webview(label) {
        let _ = webview.with_webview(move |p| {
            let _ = eepview_platform::edit(&p, native);
        });
    }
}

#[cfg(test)]
mod tests;
