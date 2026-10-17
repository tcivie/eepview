// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! `WebView2` calls through webview2-com. Every `unsafe` block holds one COM call and names
//! why it is sound.

use std::rc::Rc;

use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_COMMAND, COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SEPARATOR,
    COREWEBVIEW2_CONTEXT_MENU_TARGET_KIND, COREWEBVIEW2_CONTEXT_MENU_TARGET_KIND_IMAGE,
    COREWEBVIEW2_KEY_EVENT_KIND, COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN,
    COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
    COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL, ICoreWebView2, ICoreWebView2_11,
    ICoreWebView2_12, ICoreWebView2_22, ICoreWebView2_28,
    ICoreWebView2AcceleratorKeyPressedEventArgs, ICoreWebView2ContextMenuItem,
    ICoreWebView2ContextMenuItemCollection, ICoreWebView2ContextMenuRequestedEventArgs,
    ICoreWebView2ContextMenuTarget, ICoreWebView2Environment9, ICoreWebView2Environment15,
    ICoreWebView2Find, ICoreWebView2FindOptions, ICoreWebView2Settings4, ICoreWebView2Settings8,
};
use webview2_com::{
    AcceleratorKeyPressedEventHandler, CallDevToolsProtocolMethodCompletedHandler,
    ContextMenuRequestedEventHandler, CustomItemSelectedEventHandler, FindStartCompletedHandler,
    StatusBarTextChangedEventHandler, WebResourceRequestedEventHandler, take_pwstr,
};
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::Com::IStream;
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, MAPVK_VK_TO_CHAR, MapVirtualKeyW, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU,
    VK_RWIN, VK_SHIFT,
};
use windows_core::{BOOL, HSTRING, Interface, PWSTR, w};

use crate::{
    FindRequest, Hit, Hooks, Input, Keys, MenuEntry, Native, Nav, PlatformWebview, Reply, Rules,
    WindowButtons, code_of_char, code_of_virtual_key,
};

/// The input hook, shared by the engine handlers of one webview.
type InputHook = Rc<dyn Fn(Input) -> Reply>;
/// The chosen-item hook, shared by the custom items of every menu of one webview.
type ChosenHook = Rc<dyn Fn(&str)>;

fn core(webview: &PlatformWebview) -> Result<ICoreWebView2, String> {
    let controller = webview.controller();
    // SAFETY: the controller is live while `with_webview` runs; COM call on the UI thread.
    unsafe { controller.CoreWebView2() }.map_err(|e| e.to_string())
}

/// Attaches the request filter, then calls `done`.
pub fn attach_rules(
    webview: &PlatformWebview,
    rules: Rules<'_>,
    done: Box<dyn FnOnce(Result<(), String>)>,
) {
    done(filter(webview, rules.allow));
}

/// Answers 403 to every request whose URL `allow` refuses (L3b). The filter covers every
/// request source: the documents, and the `SharedWorker` and `ServiceWorker` requests that a
/// filter for documents only never sees. A runtime without `ICoreWebView2_22` gets no filter,
/// so the tab loads nothing (fail closed).
fn filter(webview: &PlatformWebview, allow: Box<dyn Fn(&str) -> bool>) -> Result<(), String> {
    let core = core(webview)?;
    let core22 = core
        .cast::<ICoreWebView2_22>()
        .map_err(|e| e.to_string())?;
    let env = webview.environment();
    // SAFETY: COM call on a live object with a static wide string.
    unsafe {
        core22.AddWebResourceRequestedFilterWithRequestSourceKinds(
            w!("*"),
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
            COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL,
        )
    }
    .map_err(|e| e.to_string())?;
    let handler = WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else { return Ok(()) };
        // SAFETY: getter on the live event args.
        let request = unsafe { args.Request() }?;
        let mut uri = PWSTR::null();
        // SAFETY: getter on the live request; WebView2 allocates the string we take.
        unsafe { request.Uri(&raw mut uri) }?;
        if allow(&take_pwstr(uri)) {
            return Ok(());
        }
        // SAFETY: COM call on the live environment with static strings.
        let response =
            unsafe { env.CreateWebResourceResponse(None, 403, w!("Forbidden"), w!("")) }?;
        // SAFETY: setter on the live event args.
        unsafe { args.SetResponse(&response) }
    }));
    let mut token = 0_i64;
    // SAFETY: COM call on a live object; the token outlives the call.
    unsafe { core.add_WebResourceRequested(&handler, &raw mut token) }.map_err(|e| e.to_string())
}

/// The link under the mouse, from `StatusBarTextChanged`.
///
/// # Errors
///
/// Fails when the engine handle is missing or the engine refuses the call.
pub fn on_hover(
    webview: &PlatformWebview,
    callback: Box<dyn Fn(Option<String>)>,
) -> Result<(), String> {
    let core12 = core(webview)?
        .cast::<ICoreWebView2_12>()
        .map_err(|e| e.to_string())?;
    let source = core12.clone();
    let handler = StatusBarTextChangedEventHandler::create(Box::new(move |_, _| {
        let mut text = PWSTR::null();
        // SAFETY: getter on the live webview; WebView2 allocates the string we take.
        unsafe { source.StatusBarText(&raw mut text) }?;
        callback(crate::hover_target(&take_pwstr(text)));
        Ok(())
    }));
    let mut token = 0_i64;
    // SAFETY: COM call on a live object; the token outlives the call.
    unsafe { core12.add_StatusBarTextChanged(&handler, &raw mut token) }.map_err(|e| e.to_string())
}

/// One step on the engine's navigation list.
///
/// # Errors
///
/// Fails when the engine handle is missing or the engine refuses the call.
pub fn go(webview: &PlatformWebview, nav: Nav) -> Result<(), String> {
    let core = core(webview)?;
    let result = match nav {
        // SAFETY: plain COM navigation calls on a live object on the UI thread.
        Nav::Back => unsafe { core.GoBack() },
        // SAFETY: as above.
        Nav::Forward => unsafe { core.GoForward() },
        // SAFETY: as above.
        Nav::Reload => unsafe { core.Reload() },
        Nav::HardReload => hard_reload(&core),
        // SAFETY: as above.
        Nav::Stop => unsafe { core.Stop() },
    };
    result.map_err(|e| e.to_string())
}

/// A reload that skips the cache: `ICoreWebView2::Reload` uses it, the `DevTools` call does
/// not.
fn hard_reload(core: &ICoreWebView2) -> windows_core::Result<()> {
    let done = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(|_, _| Ok(())));
    // SAFETY: COM call on a live object on the UI thread, with static wide strings; WebView2
    // keeps the handler until it calls it once.
    unsafe {
        core.CallDevToolsProtocolMethod(w!("Page.reload"), w!(r#"{"ignoreCache":true}"#), &done)
    }
}

/// The native find (`ICoreWebView2Find`): the query never reaches a page script, and a page
/// cannot change the count. A fresh search starts a find session and reports its match
/// count; next and previous move in that session. False on a runtime without the find API:
/// then nothing is searched, and no page script stands in.
#[must_use]
pub fn find(
    webview: &PlatformWebview,
    request: &FindRequest,
    on_count: Box<dyn Fn(Option<u32>)>,
) -> bool {
    let Ok(find) = finder(webview) else {
        return false;
    };
    let sent = if request.fresh {
        start_find(webview, &find, request, on_count)
    } else if request.backwards {
        // SAFETY: COM call on the live find session of this webview, on the UI thread.
        unsafe { find.FindPrevious() }
    } else {
        // SAFETY: as above.
        unsafe { find.FindNext() }
    };
    sent.is_ok()
}

/// The find API of the webview. An old runtime has no `ICoreWebView2_28`.
fn finder(webview: &PlatformWebview) -> Result<ICoreWebView2Find, String> {
    let core28 = core(webview)?
        .cast::<ICoreWebView2_28>()
        .map_err(|e| e.to_string())?;
    // SAFETY: getter on a live object on the UI thread.
    unsafe { core28.Find() }.map_err(|e| e.to_string())
}

/// Starts a find session for `request`. When it has started, `on_count` gets its match
/// count.
fn start_find(
    webview: &PlatformWebview,
    find: &ICoreWebView2Find,
    request: &FindRequest,
    on_count: Box<dyn Fn(Option<u32>)>,
) -> windows_core::Result<()> {
    let options = find_options(webview, request)?;
    let session = find.clone();
    let done = FindStartCompletedHandler::create(Box::new(move |result| {
        on_count(result.ok().and_then(|()| match_count(&session)));
        Ok(())
    }));
    // SAFETY: COM call on live objects on the UI thread; WebView2 keeps the handler until it
    // calls it once.
    unsafe { find.Start(&options, &done) }
}

/// The options of a find: the query and the case rule, every match marked, and no find bar
/// of the engine's own (eepview shows its own find bar).
fn find_options(
    webview: &PlatformWebview,
    request: &FindRequest,
) -> windows_core::Result<ICoreWebView2FindOptions> {
    let env = webview.environment().cast::<ICoreWebView2Environment15>()?;
    // SAFETY: factory call on the live environment.
    let options = unsafe { env.CreateFindOptions() }?;
    let term = HSTRING::from(request.query.as_str());
    // SAFETY: setter on the live options; the term outlives the call.
    unsafe { options.SetFindTerm(&term) }?;
    // SAFETY: setter on the live options.
    unsafe { options.SetIsCaseSensitive(request.case_sensitive) }?;
    // SAFETY: setter on the live options.
    unsafe { options.SetShouldHighlightAllMatches(true) }?;
    // SAFETY: setter on the live options.
    unsafe { options.SetSuppressDefaultFindDialog(true) }?;
    Ok(options)
}

/// The match count of a find session, or `None` when the engine has none.
fn match_count(find: &ICoreWebView2Find) -> Option<u32> {
    let mut count = 0_i32;
    // SAFETY: getter on the live find session.
    unsafe { find.MatchCount(&raw mut count) }.ok()?;
    u32::try_from(count).ok()
}

/// Ends the find session: the marks of the matches go away.
pub fn find_clear(webview: &PlatformWebview) {
    if let Ok(find) = finder(webview) {
        // SAFETY: COM call on the live find session, on the UI thread. An error means that no
        // session runs.
        drop(unsafe { find.Stop() });
    }
}

/// Autofill, password saving and `SmartScreen` reputation checks off.
///
/// # Errors
///
/// Fails when the engine handle is missing or the engine refuses the call.
pub fn harden(webview: &PlatformWebview) -> Result<(), String> {
    let core = core(webview)?;
    // SAFETY: getter on a live object.
    let settings = unsafe { core.Settings() }.map_err(|e| e.to_string())?;
    let s4 = settings
        .cast::<ICoreWebView2Settings4>()
        .map_err(|e| e.to_string())?;
    // SAFETY: setter on live settings.
    unsafe { s4.SetIsGeneralAutofillEnabled(false) }.map_err(|e| e.to_string())?;
    // SAFETY: setter on live settings.
    unsafe { s4.SetIsPasswordAutosaveEnabled(false) }.map_err(|e| e.to_string())?;
    let s8 = settings
        .cast::<ICoreWebView2Settings8>()
        .map_err(|e| e.to_string())?;
    // SAFETY: setter on live settings.
    unsafe { s8.SetIsReputationCheckingRequired(false) }.map_err(|e| e.to_string())
}

/// A native title bar: no buttons to place.
#[must_use]
pub fn place_window_buttons(_webview: &PlatformWebview, _center_y: f64) -> Option<WindowButtons> {
    None
}

/// Reports key presses (`AcceleratorKeyPressed`) and context menu requests
/// (`ContextMenuRequested`). `WebView2` hands the host every key with Ctrl or Alt, every
/// function key and Esc; the menu bar never sees them while a webview has the focus.
///
/// # Errors
///
/// Fails when the engine handle is missing or the engine refuses the call.
pub fn on_input(webview: &PlatformWebview, hooks: Hooks) -> Result<(), String> {
    let Hooks { input, chosen, .. } = hooks;
    let input: InputHook = Rc::from(input);
    on_keys(webview, Rc::clone(&input)).map_err(|e| e.to_string())?;
    on_menu(webview, input, Rc::from(chosen)).map_err(|e| e.to_string())
}

fn on_keys(webview: &PlatformWebview, input: InputHook) -> windows_core::Result<()> {
    let handler = AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
        args.map_or(Ok(()), |args| key_pressed(&args, &input))
    }));
    let mut token = 0_i64;
    // SAFETY: COM call on the live controller; the token outlives the call.
    unsafe {
        webview
            .controller()
            .add_AcceleratorKeyPressed(&handler, &raw mut token)
    }
}

/// One key event: a key down that the app takes is marked handled, so the page and the
/// engine never see it.
fn key_pressed(
    args: &ICoreWebView2AcceleratorKeyPressedEventArgs,
    input: &InputHook,
) -> windows_core::Result<()> {
    let mut kind = COREWEBVIEW2_KEY_EVENT_KIND::default();
    // SAFETY: getter on the live event args.
    unsafe { args.KeyEventKind(&raw mut kind) }?;
    if kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN
        && kind != COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN
    {
        return Ok(());
    }
    let mut vk = 0_u32;
    // SAFETY: getter on the live event args.
    unsafe { args.VirtualKey(&raw mut vk) }?;
    let Some(code) = key_code(vk) else {
        return Ok(());
    };
    if input(Input::Key {
        code,
        keys: held_keys(),
    }) == Reply::Consume
    {
        // SAFETY: setter on the live event args.
        unsafe { args.SetHandled(true) }?;
    }
    Ok(())
}

/// The `KeyboardEvent.code` name of a virtual key on the active layout.
fn key_code(vk: u32) -> Option<String> {
    code_of_virtual_key(vk).or_else(|| {
        // SAFETY: a pure table lookup in the active keyboard layout.
        let mapped = unsafe { MapVirtualKeyW(vk, MAPVK_VK_TO_CHAR) };
        // The top bit marks a dead key; the low word is the character.
        char::from_u32(mapped & 0xFFFF).and_then(code_of_char)
    })
}

fn down(key: VIRTUAL_KEY) -> bool {
    // SAFETY: reads the key state of this thread's input queue.
    let state = unsafe { GetKeyState(i32::from(key.0)) };
    state < 0
}

/// The modifier keys held now, from the input state of the UI thread.
#[must_use]
pub fn held_keys() -> Keys {
    Keys::default()
        .with_meta(down(VK_LWIN) || down(VK_RWIN))
        .with_ctrl(down(VK_CONTROL))
        .with_alt(down(VK_MENU))
        .with_shift(down(VK_SHIFT))
}

fn on_menu(
    webview: &PlatformWebview,
    input: InputHook,
    chosen: ChosenHook,
) -> windows_core::Result<()> {
    let core11 = core(webview)
        .map_err(|e| windows_core::Error::new(windows_core::HRESULT(-1), e))?
        .cast::<ICoreWebView2_11>()?;
    let env = webview.environment().cast::<ICoreWebView2Environment9>()?;
    let handler = ContextMenuRequestedEventHandler::create(Box::new(move |_, args| {
        args.map_or(Ok(()), |args| menu_requested(&args, &env, &input, &chosen))
    }));
    let mut token = 0_i64;
    // SAFETY: COM call on a live object; the token outlives the call.
    unsafe { core11.add_ContextMenuRequested(&handler, &raw mut token) }
}

/// Replaces the engine menu with the app's entries. No entries: no menu at all.
fn menu_requested(
    args: &ICoreWebView2ContextMenuRequestedEventArgs,
    env: &ICoreWebView2Environment9,
    input: &InputHook,
    chosen: &ChosenHook,
) -> windows_core::Result<()> {
    // SAFETY: getter on the live event args.
    let target = unsafe { args.ContextMenuTarget() }?;
    let Reply::Menu(entries) = input(Input::Menu(hit_of(&target)?)) else {
        return Ok(());
    };
    if entries.is_empty() {
        // SAFETY: setter on the live event args; a handled request shows no menu.
        return unsafe { args.SetHandled(true) };
    }
    // SAFETY: getter on the live event args.
    let items = unsafe { args.MenuItems() }?;
    let natives = take_items(&items)?;
    for (index, entry) in (0_u32..).zip(&entries) {
        let item = menu_item(entry, &natives, env, chosen)?;
        // SAFETY: COM call on the live collection with a live item.
        unsafe { items.InsertValueAtIndex(index, &item) }?;
    }
    Ok(())
}

fn read_bool(
    get: impl FnOnce(*mut BOOL) -> windows_core::Result<()>,
) -> windows_core::Result<bool> {
    let mut value = BOOL::default();
    get(&raw mut value)?;
    Ok(value.as_bool())
}

fn read_text(
    get: impl FnOnce(*mut PWSTR) -> windows_core::Result<()>,
) -> windows_core::Result<String> {
    let mut value = PWSTR::null();
    get(&raw mut value)?;
    Ok(take_pwstr(value))
}

/// What is under the pointer, from the engine's menu target.
fn hit_of(target: &ICoreWebView2ContextMenuTarget) -> windows_core::Result<Hit> {
    // SAFETY: getter on the live target.
    let has_link = read_bool(|v| unsafe { target.HasLinkUri(v) })?;
    // SAFETY: getter on the live target; WebView2 allocates the string we take.
    let link = read_text(|v| unsafe { target.LinkUri(v) })?;
    let mut kind = COREWEBVIEW2_CONTEXT_MENU_TARGET_KIND::default();
    // SAFETY: getter on the live target.
    unsafe { target.Kind(&raw mut kind) }?;
    // SAFETY: getter on the live target.
    let has_source = read_bool(|v| unsafe { target.HasSourceUri(v) })?;
    // SAFETY: getter on the live target; WebView2 allocates the string we take.
    let source = read_text(|v| unsafe { target.SourceUri(v) })?;
    let image = kind == COREWEBVIEW2_CONTEXT_MENU_TARGET_KIND_IMAGE && has_source;
    Ok(Hit {
        link: has_link.then_some(link).filter(|l| !l.is_empty()),
        image: image.then_some(source).filter(|s| !s.is_empty()),
        // SAFETY: getter on the live target.
        selection: read_bool(|v| unsafe { target.HasSelection(v) })?,
        // SAFETY: getter on the live target.
        editable: read_bool(|v| unsafe { target.IsEditable(v) })?,
    })
}

/// Empties the engine menu and returns its items by name, so the app's menu can reuse the
/// engine's own editing items.
fn take_items(
    items: &ICoreWebView2ContextMenuItemCollection,
) -> windows_core::Result<Vec<(String, ICoreWebView2ContextMenuItem)>> {
    let mut count = 0_u32;
    // SAFETY: getter on the live collection.
    unsafe { items.Count(&raw mut count) }?;
    let mut named = Vec::new();
    for _ in 0..count {
        // SAFETY: index 0 exists while the count is above zero.
        let item = unsafe { items.GetValueAtIndex(0) }?;
        // SAFETY: getter on the live item; WebView2 allocates the string we take.
        let name = read_text(|v| unsafe { item.Name(v) })?;
        // SAFETY: index 0 exists; the item stays alive in `named`.
        unsafe { items.RemoveValueAtIndex(0) }?;
        named.push((name, item));
    }
    Ok(named)
}

/// The engine's name of the item that does a native command.
fn native_name(native: Native) -> &'static str {
    match native {
        Native::Undo => "undo",
        Native::Redo => "redo",
        Native::Cut => "cut",
        Native::Copy => "copy",
        Native::Paste => "paste",
        Native::SelectAll => "selectAll",
        Native::CopyImage => "copyImage",
    }
}

fn menu_item(
    entry: &MenuEntry,
    natives: &[(String, ICoreWebView2ContextMenuItem)],
    env: &ICoreWebView2Environment9,
    chosen: &ChosenHook,
) -> windows_core::Result<ICoreWebView2ContextMenuItem> {
    let MenuEntry::Item {
        id,
        label,
        enabled,
        native,
    } = entry
    else {
        // SAFETY: COM factory call on the live environment with a static string.
        return unsafe {
            env.CreateContextMenuItem(
                w!(""),
                None::<&IStream>,
                COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SEPARATOR,
            )
        };
    };
    let engine_item = native.and_then(|n| natives.iter().find(|(name, _)| name == native_name(n)));
    if let Some((_, item)) = engine_item {
        return Ok(item.clone());
    }
    // A native command the engine did not offer here cannot run: it shows, disabled.
    let enabled = *enabled && native.is_none();
    custom_item(id, label, enabled, env, chosen)
}

fn custom_item(
    id: &str,
    label: &str,
    enabled: bool,
    env: &ICoreWebView2Environment9,
    chosen: &ChosenHook,
) -> windows_core::Result<ICoreWebView2ContextMenuItem> {
    let text = HSTRING::from(label);
    // SAFETY: COM factory call on the live environment; the label outlives the call.
    let item = unsafe {
        env.CreateContextMenuItem(
            &text,
            None::<&IStream>,
            COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_COMMAND,
        )
    }?;
    // SAFETY: setter on the live item.
    unsafe { item.SetIsEnabled(enabled) }?;
    let (hook, id) = (Rc::clone(chosen), id.to_owned());
    let handler = CustomItemSelectedEventHandler::create(Box::new(move |_, _| {
        hook(&id);
        Ok(())
    }));
    let mut token = 0_i64;
    // SAFETY: COM call on the live item; the token outlives the call.
    unsafe { item.add_CustomItemSelected(&handler, &raw mut token) }?;
    Ok(item)
}

/// Puts `text` on the clipboard as Unicode text.
///
/// # Errors
///
/// Fails when the clipboard is busy or the copy cannot be made.
pub fn copy_text(text: &str) -> Result<(), String> {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: opens the clipboard for this thread; it is closed below on every path.
    unsafe { OpenClipboard(None) }.map_err(|e| e.to_string())?;
    let result = fill_clipboard(&wide);
    // SAFETY: the clipboard is open on this thread.
    let closed = unsafe { CloseClipboard() };
    result.and(closed.map_err(|e| e.to_string()))
}

/// Writes `wide` (with its final 0) to the open clipboard.
fn fill_clipboard(wide: &[u16]) -> Result<(), String> {
    // SAFETY: the caller opened the clipboard on this thread.
    unsafe { EmptyClipboard() }.map_err(|e| e.to_string())?;
    // SAFETY: allocates movable global memory of the given size.
    let memory = unsafe { GlobalAlloc(GMEM_MOVEABLE, std::mem::size_of_val(wide)) }
        .map_err(|e| e.to_string())?;
    let written = write_global(memory, wide);
    let handed = written.and_then(|()| {
        // SAFETY: the clipboard is open and `memory` holds a 0-terminated UTF-16 string;
        // on success the clipboard owns the memory.
        unsafe { SetClipboardData(u32::from(CF_UNICODETEXT.0), Some(HANDLE(memory.0))) }
            .map(drop)
            .map_err(|e| e.to_string())
    });
    if handed.is_err() {
        // SAFETY: the clipboard did not take `memory`, so it is still ours to free.
        drop(unsafe { GlobalFree(Some(memory)) });
    }
    handed
}

fn write_global(memory: HGLOBAL, wide: &[u16]) -> Result<(), String> {
    // SAFETY: `memory` is a live movable block; locking gives its address.
    let target = unsafe { GlobalLock(memory) }.cast::<u16>();
    if target.is_null() {
        return Err("the clipboard memory could not be locked".into());
    }
    // SAFETY: the block holds `wide.len()` u16 values (its size was made from `wide`), and
    // the two regions do not overlap.
    unsafe { std::ptr::copy_nonoverlapping(wide.as_ptr(), target, wide.len()) };
    // SAFETY: unlocks the block locked above. An error here only means it is unlocked.
    drop(unsafe { GlobalUnlock(memory) });
    Ok(())
}

/// The editing commands run from the engine's own menu items: nothing to do here.
///
/// # Errors
///
/// Always: `WebView2` runs these commands from its own menu items.
pub fn edit(_webview: &PlatformWebview, _command: Native) -> Result<(), String> {
    Err("WebView2 runs editing commands from its own menu items".into())
}

/// No system menu items to take out.
pub fn quiet_menus() {}
