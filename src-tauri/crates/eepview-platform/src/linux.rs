// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! `WebKitGTK` calls. The webkit2gtk crate wraps them safely, so this module has no `unsafe`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::gdk::{self, EventButton, EventKey, EventType, ModifierType};
use gtk::glib::Propagation;
use gtk::prelude::WidgetExt;
use webkit2gtk::gio::SimpleAction;
use webkit2gtk::glib::ObjectExt;
use webkit2gtk::{
    ContextMenu, ContextMenuAction, ContextMenuExt, ContextMenuItem, FindControllerExt,
    FindOptions, HitTestResult, HitTestResultExt, SettingsExt, WebView, WebViewExt,
};

use crate::{
    Button, FindRequest, Hit, Hooks, Input, Keys, MenuEntry, Native, Nav, PlatformWebview, Reply,
    Rules, WindowButtons,
};

/// The input hook, shared by the signal handlers of one webview.
type InputHook = Rc<dyn Fn(Input) -> Reply>;
/// The chosen-item hook, shared by the items of every menu of one webview.
type ChosenHook = Rc<dyn Fn(&str)>;

/// No request filter: webkit2gtk has no binding for one, and the engine makes no requests
/// of its own (spike S1). The CSP of the gatekeeper is the L3 layer here.
pub fn attach_rules(
    _webview: &PlatformWebview,
    rules: Rules<'_>,
    done: Box<dyn FnOnce(Result<(), String>)>,
) {
    drop(rules);
    done(Ok(()));
}

/// The link under the mouse, from `mouse-target-changed`.
///
/// # Errors
///
/// Fails when the engine handle is missing or the engine refuses the call.
pub fn on_hover(
    webview: &PlatformWebview,
    callback: Box<dyn Fn(Option<String>)>,
) -> Result<(), String> {
    webview
        .inner()
        .connect_mouse_target_changed(move |_, hit, _| {
            let link = hit.context_is_link().then(|| hit.link_uri()).flatten();
            callback(link.map(|s| s.as_str().to_owned()));
        });
    Ok(())
}

/// One step on the engine's navigation list.
///
/// # Errors
///
/// Fails when the engine handle is missing or the engine refuses the call.
pub fn go(webview: &PlatformWebview, nav: Nav) -> Result<(), String> {
    let view = webview.inner();
    match nav {
        Nav::Back => view.go_back(),
        Nav::Forward => view.go_forward(),
        Nav::Reload => view.reload(),
        Nav::HardReload => view.reload_bypass_cache(),
        Nav::Stop => view.stop_loading(),
    }
    Ok(())
}

/// `WebKitFindController`: a fresh search counts the matches once.
#[must_use]
pub fn find(
    webview: &PlatformWebview,
    request: &FindRequest,
    on_count: Box<dyn Fn(Option<u32>)>,
) -> bool {
    let Some(controller) = webview.inner().find_controller() else {
        return false;
    };
    let mut options = FindOptions::WRAP_AROUND;
    if !request.case_sensitive {
        options |= FindOptions::CASE_INSENSITIVE;
    }
    if request.backwards {
        options |= FindOptions::BACKWARDS;
    }
    if !request.fresh {
        if request.backwards {
            controller.search_previous();
        } else {
            controller.search_next();
        }
        return true;
    }
    let slot: Rc<Cell<Option<webkit2gtk::glib::SignalHandlerId>>> = Rc::new(Cell::new(None));
    let own = Rc::clone(&slot);
    let id = controller.connect_counted_matches(move |c, n| {
        on_count(Some(n));
        if let Some(id) = own.take() {
            c.disconnect(id);
        }
    });
    slot.set(Some(id));
    controller.count_matches(&request.query, options.bits(), u32::MAX);
    controller.search(&request.query, options.bits(), u32::MAX);
    true
}

/// Ends a find.
pub fn find_clear(webview: &PlatformWebview) {
    if let Some(controller) = webview.inner().find_controller() {
        controller.search_finish();
    }
}

/// WebRTC and media capture off in the engine settings (L5).
///
/// # Errors
///
/// Fails when the engine handle is missing or the engine refuses the call.
pub fn harden(webview: &PlatformWebview) -> Result<(), String> {
    let settings = WebViewExt::settings(&webview.inner()).ok_or("no WebKit settings")?;
    settings.set_enable_webrtc(false);
    settings.set_enable_media_stream(false);
    Ok(())
}

/// A native title bar: no buttons to place.
#[must_use]
pub fn place_window_buttons(_webview: &PlatformWebview, _center_y: f64) -> Option<WindowButtons> {
    None
}

/// Reports context menu requests (`context-menu`) on every webview and, on a page webview,
/// modified clicks and middle-clicks on links and the mouse back and forward buttons
/// (`button-press-event`), and Esc (`key-press-event`). The menu bar takes the shortcuts
/// before the webview sees them.
///
/// # Errors
///
/// Never: the signals always connect.
pub fn on_input(webview: &PlatformWebview, hooks: Hooks) -> Result<(), String> {
    let view = webview.inner();
    let Hooks {
        content,
        input,
        chosen,
    } = hooks;
    let input: InputHook = Rc::from(input);
    let chosen: ChosenHook = Rc::from(chosen);
    let menu_input = Rc::clone(&input);
    view.connect_context_menu(move |_, menu, _, hit| {
        menu_requested(menu, hit, &menu_input, &chosen)
    });
    if content {
        page_hooks(&view, input);
    }
    Ok(())
}

fn page_hooks(view: &WebView, input: InputHook) {
    let hovered: Rc<RefCell<Option<String>>> = Rc::default();
    let seen = Rc::clone(&hovered);
    view.connect_mouse_target_changed(move |_, hit, _| {
        *seen.borrow_mut() = link_of(hit);
    });
    let press = Rc::clone(&input);
    view.connect_button_press_event(move |_, event| pressed(event, &hovered, &press));
    view.connect_key_press_event(move |_, event| key_pressed(event, &input));
}

fn link_of(hit: &HitTestResult) -> Option<String> {
    hit.context_is_link()
        .then(|| hit.link_uri())
        .flatten()
        .map(|s| s.as_str().to_owned())
}

fn keys_of(state: ModifierType) -> Keys {
    Keys::default()
        .with_meta(state.contains(ModifierType::SUPER_MASK))
        .with_ctrl(state.contains(ModifierType::CONTROL_MASK))
        .with_alt(state.contains(ModifierType::MOD1_MASK))
        .with_shift(state.contains(ModifierType::SHIFT_MASK))
}

/// A button press: the back and forward buttons, and a click on the link under the
/// pointer. An event the app takes never reaches the engine.
fn pressed(
    event: &EventButton,
    hovered: &RefCell<Option<String>>,
    input: &InputHook,
) -> Propagation {
    if event.event_type() != EventType::ButtonPress {
        return Propagation::Proceed;
    }
    let report = match event.button() {
        8 => Some(Input::Mouse(Button::Back)),
        9 => Some(Input::Mouse(Button::Forward)),
        n @ (1 | 2) => hovered.borrow().clone().map(|url| Input::Link {
            url,
            keys: keys_of(event.state()),
            button: if n == 2 {
                Button::Middle
            } else {
                Button::Primary
            },
        }),
        _ => None,
    };
    if report.is_some_and(|event| input(event) == Reply::Consume) {
        Propagation::Stop
    } else {
        Propagation::Proceed
    }
}

/// Esc goes to the app and then on to the page.
fn key_pressed(event: &EventKey, input: &InputHook) -> Propagation {
    if event.keyval() == gdk::keys::constants::Escape {
        drop(input(Input::Key {
            code: "Escape".into(),
            keys: keys_of(event.state()),
        }));
    }
    Propagation::Proceed
}

/// Replaces the engine menu with the app's entries; no entries cancels the menu.
fn menu_requested(
    menu: &ContextMenu,
    hit: &HitTestResult,
    input: &InputHook,
    chosen: &ChosenHook,
) -> bool {
    let found = Hit {
        link: link_of(hit),
        image: hit
            .context_is_image()
            .then(|| hit.image_uri())
            .flatten()
            .map(|s| s.as_str().to_owned()),
        selection: hit.context_is_selection(),
        editable: hit.context_is_editable(),
    };
    let Reply::Menu(entries) = input(Input::Menu(found)) else {
        return false;
    };
    menu.remove_all();
    for (index, entry) in entries.iter().enumerate() {
        menu.append(&menu_item(index, entry, chosen));
    }
    entries.is_empty()
}

/// The engine action that runs a native command with its own item, where one exists.
fn stock(native: Native) -> Option<ContextMenuAction> {
    match native {
        Native::Cut => Some(ContextMenuAction::Cut),
        Native::Copy => Some(ContextMenuAction::Copy),
        Native::Paste => Some(ContextMenuAction::Paste),
        Native::SelectAll => Some(ContextMenuAction::SelectAll),
        Native::CopyImage => Some(ContextMenuAction::CopyImageToClipboard),
        Native::Undo | Native::Redo => None,
    }
}

fn menu_item(index: usize, entry: &MenuEntry, chosen: &ChosenHook) -> ContextMenuItem {
    let MenuEntry::Item {
        id,
        label,
        enabled,
        native,
    } = entry
    else {
        return ContextMenuItem::new_separator();
    };
    if let Some(action) = native.and_then(stock) {
        return ContextMenuItem::from_stock_action_with_label(action, label);
    }
    // A `GAction` name allows letters, digits, `-` and `.` only, so the index names it.
    let action = SimpleAction::new(&format!("eepview-menu-{index}"), None);
    action.set_enabled(*enabled);
    let (hook, id) = (Rc::clone(chosen), id.clone());
    action.connect_activate(move |_, _| hook(&id));
    ContextMenuItem::from_gaction(&action, label, None)
}

/// Puts `text` on the clipboard.
///
/// # Errors
///
/// Fails off the GTK main thread.
pub fn copy_text(text: &str) -> Result<(), String> {
    if !gtk::is_initialized_main_thread() {
        return Err("not on the GTK main thread".into());
    }
    gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD).set_text(text);
    Ok(())
}

/// An editing command, through the engine's editing command names.
///
/// # Errors
///
/// Never: the engine ignores a command it cannot run.
pub fn edit(webview: &PlatformWebview, command: Native) -> Result<(), String> {
    let name = match command {
        Native::Undo => "Undo",
        Native::Redo => "Redo",
        Native::Cut => "Cut",
        Native::Copy | Native::CopyImage => "Copy",
        Native::Paste => "Paste",
        Native::SelectAll => "SelectAll",
    };
    webview.inner().execute_editing_command(name);
    Ok(())
}

/// The modifier keys held now, from the keymap of the default display.
#[must_use]
pub fn held_keys() -> Keys {
    if !gtk::is_initialized_main_thread() {
        return Keys::default();
    }
    let state = gdk::Display::default()
        .and_then(|d| gdk::Keymap::for_display(&d))
        .map_or(0, |k| k.modifier_state());
    keys_of(ModifierType::from_bits_truncate(state))
}

/// No system menu items to take out.
pub fn quiet_menus() {}
