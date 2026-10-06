// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! `WKWebView` calls (macOS 14+). Every `unsafe` block holds one call into Objective-C and
//! names why it is sound.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ptr::NonNull;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, ProtocolObject, Sel};
use objc2::{DefinedClass, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_foundation::{
    MainThreadMarker, NSError, NSNumber, NSObjectProtocol, NSString, NSURL, NSUserDefaults,
};
use objc2_web_kit::{
    WKContentRuleList, WKContentRuleListStore, WKContentWorld, WKFindConfiguration, WKFindResult,
    WKNavigation, WKNavigationDelegate, WKScriptMessage, WKScriptMessageHandler,
    WKUserContentController, WKUserScript, WKUserScriptInjectionTime, WKWebView,
};

use objc2_app_kit::{
    NSApplication, NSButton, NSEvent, NSEventModifierFlags, NSPasteboard, NSPasteboardTypeString,
    NSView, NSWindow, NSWindowButton,
};
use objc2_foundation::NSRect;

use crate::{
    CLEAR_SELECTION_SCRIPT, FindRequest, HOVER_CHANNEL, HOVER_SCRIPT, Hooks, INPUT_CHANNEL, Keys,
    LoadFailure, Native, Nav, PlatformWebview, Rules, WindowButtons, count_script, hover_target,
    input_script, parse_message,
};

thread_local! {
    /// The compiled rule lists by identifier, shared by every webview that uses one rule
    /// set (main thread only).
    static RULES: RefCell<HashMap<String, Retained<WKContentRuleList>>> =
        RefCell::new(HashMap::new());
}

/// The `WKWebView` behind a Tauri webview, and the main-thread marker.
fn view(webview: &PlatformWebview) -> Result<(Retained<WKWebView>, MainThreadMarker), String> {
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let ptr = webview.inner().cast::<WKWebView>();
    // SAFETY: `PlatformWebview::inner` is the live `WKWebView` of this webview (wry creates it,
    // Tauri keeps it alive while `with_webview` runs), and we are on the main thread.
    let retained = unsafe { Retained::retain(ptr) };
    retained
        .map(|v| (v, mtm))
        .ok_or_else(|| "no WKWebView".to_owned())
}

fn content_controller(view: &WKWebView) -> Retained<WKUserContentController> {
    // SAFETY: `configuration` is a plain getter on a live view; it returns a copy we own.
    let config = unsafe { view.configuration() };
    // SAFETY: plain getter on a live configuration.
    unsafe { config.userContentController() }
}

fn world(mtm: MainThreadMarker) -> Retained<WKContentWorld> {
    // SAFETY: `worldWithName` returns the shared named world; main thread per `mtm`.
    unsafe { WKContentWorld::worldWithName(&NSString::from_str(HOVER_CHANNEL), mtm) }
}

/// Adds the compiled rule list to the view, compiling it on first use.
pub fn attach_rules(
    webview: &PlatformWebview,
    rules: Rules<'_>,
    done: Box<dyn FnOnce(Result<(), String>)>,
) {
    let (view, mtm) = match view(webview) {
        Ok(v) => v,
        Err(e) => return done(Err(e)),
    };
    let Rules { id, json, allow } = rules;
    // The allow function is the Windows filter; WebKit uses the compiled list.
    drop(allow);
    let cached = RULES.with(|r| r.borrow().get(id).cloned());
    if let Some(list) = cached {
        add_rules(&view, &list);
        return done(Ok(()));
    }
    compile(&view, id, json, mtm, done);
}

fn add_rules(view: &WKWebView, list: &WKContentRuleList) {
    let controller = content_controller(view);
    // SAFETY: both objects are live; WebKit retains the list.
    unsafe { controller.addContentRuleList(list) };
}

fn compile(
    view: &WKWebView,
    id: &str,
    json: &str,
    mtm: MainThreadMarker,
    done: Box<dyn FnOnce(Result<(), String>)>,
) {
    // SAFETY: `defaultStore` is a class getter; main thread per `mtm`.
    let Some(store) = (unsafe { WKContentRuleListStore::defaultStore(mtm) }) else {
        return done(Err("no content rule list store".into()));
    };
    let view = view.retain();
    let key = id.to_owned();
    let done = Cell::new(Some(done));
    let block = RcBlock::new(move |list: *mut WKContentRuleList, error: *mut NSError| {
        let Some(done) = done.take() else { return };
        // SAFETY: WebKit passes a valid list or null; `retain` handles null.
        match unsafe { Retained::retain(list) } {
            Some(list) => {
                add_rules(&view, &list);
                RULES.with(|r| r.borrow_mut().insert(key.clone(), list));
                done(Ok(()));
            }
            None => done(Err(error_text(error))),
        }
    });
    let id = NSString::from_str(id);
    let json = NSString::from_str(json);
    // SAFETY: all arguments are live for the call; WebKit copies the block and calls it once
    // on the main thread.
    unsafe {
        store.compileContentRuleListForIdentifier_encodedContentRuleList_completionHandler(
            Some(&id),
            Some(&json),
            Some(&block),
        );
    }
}

fn error_text(error: *mut NSError) -> String {
    // SAFETY: WebKit passes a valid error or null; `retain` handles null.
    let error = unsafe { Retained::retain(error) };
    error.map_or_else(
        || "rule list did not compile".into(),
        |e| e.localizedDescription().to_string(),
    )
}

/// The ivars of [`HoverHandler`].
pub struct HoverIvars {
    callback: Box<dyn Fn(Option<String>)>,
}

define_class!(
    // SAFETY: NSObject has no subclassing rules; the class adds no Drop impl and only
    // reads its ivars on the main thread.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = HoverIvars]
    struct HoverHandler;

    // SAFETY: NSObjectProtocol has no required methods.
    unsafe impl NSObjectProtocol for HoverHandler {}

    // SAFETY: the method signature matches `userContentController:didReceiveScriptMessage:`.
    unsafe impl WKScriptMessageHandler for HoverHandler {
        #[unsafe(method(userContentController:didReceiveScriptMessage:))]
        fn did_receive(&self, _controller: &WKUserContentController, message: &WKScriptMessage) {
            // SAFETY: `body` is a plain getter on a live message.
            let body = unsafe { message.body() };
            let text = body
                .downcast::<NSString>()
                .map(|s| s.to_string())
                .unwrap_or_default();
            (self.ivars().callback)(hover_target(&text));
        }
    }
);

impl HoverHandler {
    fn new(callback: Box<dyn Fn(Option<String>)>, mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(HoverIvars { callback });
        // SAFETY: `init` of NSObject on a freshly allocated instance with ivars set.
        unsafe { msg_send![super(this), init] }
    }
}

/// Installs the hover script in the private world, in every frame, and its message handler.
///
/// # Errors
///
/// Fails when the engine handle is missing or the engine refuses the call.
pub fn on_hover(
    webview: &PlatformWebview,
    callback: Box<dyn Fn(Option<String>)>,
) -> Result<(), String> {
    let (view, mtm) = view(webview)?;
    let controller = content_controller(&view);
    let world = world(mtm);
    add_script(&controller, &world, HOVER_SCRIPT, mtm);
    let handler = HoverHandler::new(callback, mtm);
    let name = NSString::from_str(HOVER_CHANNEL);
    // SAFETY: the controller retains the handler; the name is unique in this world.
    unsafe {
        controller.addScriptMessageHandler_contentWorld_name(
            ProtocolObject::from_ref(&*handler),
            &world,
            &name,
        );
    }
    Ok(())
}

/// Adds a script that runs at document start in every frame of `world`.
fn add_script(
    controller: &WKUserContentController,
    world: &WKContentWorld,
    source: &str,
    mtm: MainThreadMarker,
) {
    let source = NSString::from_str(source);
    // SAFETY: designated initializer on a fresh allocation; all arguments are live.
    let script = unsafe {
        WKUserScript::initWithSource_injectionTime_forMainFrameOnly_inContentWorld(
            mtm.alloc(),
            &source,
            WKUserScriptInjectionTime::AtDocumentStart,
            false,
            world,
        )
    };
    // SAFETY: both objects are live; the controller retains the script.
    unsafe { controller.addUserScript(&script) };
}

/// The ivars of [`InputHandler`].
pub struct InputIvars {
    hooks: Hooks,
}

define_class!(
    // SAFETY: NSObject has no subclassing rules; the class adds no Drop impl and only
    // reads its ivars on the main thread.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = InputIvars]
    struct InputHandler;

    // SAFETY: NSObjectProtocol has no required methods.
    unsafe impl NSObjectProtocol for InputHandler {}

    // SAFETY: the method signature matches `userContentController:didReceiveScriptMessage:`.
    unsafe impl WKScriptMessageHandler for InputHandler {
        #[unsafe(method(userContentController:didReceiveScriptMessage:))]
        fn did_receive(&self, _controller: &WKUserContentController, message: &WKScriptMessage) {
            // SAFETY: `body` is a plain getter on a live message.
            let body = unsafe { message.body() };
            let text = body
                .downcast::<NSString>()
                .map(|s| s.to_string())
                .unwrap_or_default();
            if let Some(input) = parse_message(&text) {
                // The app shows its own menu on macOS: the reply has nothing to change here.
                drop((self.ivars().hooks.input)(input));
            }
        }
    }
);

impl InputHandler {
    fn new(hooks: Hooks, mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(InputIvars { hooks });
        // SAFETY: `init` of NSObject on a freshly allocated instance with ivars set.
        unsafe { msg_send![super(this), init] }
    }
}

/// Installs the input script in the private world and its message handler. The script also
/// runs once now, for a page that loaded before this call (the bundled pages).
///
/// # Errors
///
/// Fails when the engine handle is missing or the engine refuses the call.
pub fn on_input(webview: &PlatformWebview, hooks: Hooks) -> Result<(), String> {
    let (view, mtm) = view(webview)?;
    let controller = content_controller(&view);
    let world = world(mtm);
    let source = input_script(hooks.content);
    add_script(&controller, &world, &source, mtm);
    let handler = InputHandler::new(hooks, mtm);
    let name = NSString::from_str(INPUT_CHANNEL);
    // SAFETY: the controller retains the handler; the name is unique in this world.
    unsafe {
        controller.addScriptMessageHandler_contentWorld_name(
            ProtocolObject::from_ref(&*handler),
            &world,
            &name,
        );
    }
    run_in_world(&view, mtm, &source, Box::new(|_| {}));
    Ok(())
}

/// Puts `text` on the general pasteboard.
///
/// # Errors
///
/// Fails off the main thread or when the pasteboard refuses the text.
pub fn copy_text(text: &str) -> Result<(), String> {
    MainThreadMarker::new().ok_or("not on the main thread")?;
    let board = NSPasteboard::generalPasteboard();
    board.clearContents();
    // SAFETY: an AppKit constant, set before `main` runs and never changed.
    let kind = unsafe { NSPasteboardTypeString };
    board
        .setString_forType(&NSString::from_str(text), kind)
        .then_some(())
        .ok_or_else(|| "the pasteboard refused the text".to_owned())
}

/// The script that selects the image of the last context menu (input script).
const SELECT_IMAGE: &str =
    "window.__eepviewSelectImage ? (window.__eepviewSelectImage() ? 1 : 0) : 0";

/// An editing command, sent to the view as the Edit menu sends it. Copy Image first selects
/// the image of the last context menu, then copies that selection.
///
/// # Errors
///
/// Fails when the engine handle is missing or no responder takes the command.
pub fn edit(webview: &PlatformWebview, command: Native) -> Result<(), String> {
    let (view, mtm) = view(webview)?;
    let action = match command {
        Native::Undo => sel!(undo:),
        Native::Redo => sel!(redo:),
        Native::Cut => sel!(cut:),
        Native::Copy => sel!(copy:),
        Native::Paste => sel!(paste:),
        Native::SelectAll => sel!(selectAll:),
        Native::CopyImage => {
            let target = view.clone();
            let copy = move |selected: Option<u32>| {
                let _ = selected == Some(1) && send_action(&target, mtm, sel!(copy:));
            };
            run_in_world(&view, mtm, SELECT_IMAGE, Box::new(copy));
            return Ok(());
        }
    };
    send_action(&view, mtm, action)
        .then_some(())
        .ok_or_else(|| "no responder took the command".to_owned())
}

/// Makes `view` the first responder, then sends `action` down the responder chain.
fn send_action(view: &WKWebView, mtm: MainThreadMarker, action: Sel) -> bool {
    if let Some(window) = view.window() {
        window.makeFirstResponder(Some(view));
    }
    let app = NSApplication::sharedApplication(mtm);
    // SAFETY: `action` is one of the standard editing selectors, which take one sender
    // argument; a nil target means the first responder and a nil sender is allowed.
    unsafe { app.sendAction_to_from(action, None, None) }
}

/// The modifier keys held now.
#[must_use]
pub fn held_keys() -> Keys {
    let flags = NSEvent::modifierFlags_class();
    Keys::default()
        .with_meta(flags.contains(NSEventModifierFlags::Command))
        .with_ctrl(flags.contains(NSEventModifierFlags::Control))
        .with_alt(flags.contains(NSEventModifierFlags::Option))
        .with_shift(flags.contains(NSEventModifierFlags::Shift))
}

/// `AppKit` adds Start Dictation and Emoji & Symbols to the Edit menu unless these user
/// defaults say no (P4).
pub fn quiet_menus() {
    let defaults = NSUserDefaults::standardUserDefaults();
    for key in [
        "NSDisabledDictationMenuItem",
        "NSDisabledCharacterPaletteMenuItem",
    ] {
        defaults.setBool_forKey(true, &NSString::from_str(key));
    }
}

/// One step on the engine's navigation list.
///
/// # Errors
///
/// Fails when the engine handle is missing or the engine refuses the call.
pub fn go(webview: &PlatformWebview, nav: Nav) -> Result<(), String> {
    let (view, _) = view(webview)?;
    match nav {
        // SAFETY: plain navigation calls on a live view on the main thread.
        Nav::Back => drop(unsafe { view.goBack() }),
        // SAFETY: as above.
        Nav::Forward => drop(unsafe { view.goForward() }),
        // SAFETY: as above.
        Nav::Reload => drop(unsafe { view.reload() }),
        // SAFETY: as above.
        Nav::HardReload => drop(unsafe { view.reloadFromOrigin() }),
        // SAFETY: as above.
        Nav::Stop => unsafe { view.stopLoading() },
    }
    Ok(())
}

/// Native find; a fresh search also counts the matches in the private world.
#[must_use]
pub fn find(
    webview: &PlatformWebview,
    request: &FindRequest,
    on_count: Box<dyn Fn(Option<u32>)>,
) -> bool {
    let Ok((view, mtm)) = view(webview) else {
        return false;
    };
    if request.fresh {
        run_in_world(
            &view,
            mtm,
            &count_script(&request.query, request.case_sensitive),
            on_count,
        );
    }
    // SAFETY: plain constructor; main thread per `mtm`.
    let config = unsafe { WKFindConfiguration::new(mtm) };
    // SAFETY: setter on a live configuration.
    unsafe { config.setBackwards(request.backwards) };
    // SAFETY: setter on a live configuration.
    unsafe { config.setCaseSensitive(request.case_sensitive) };
    // SAFETY: setter on a live configuration.
    unsafe { config.setWraps(true) };
    let done = RcBlock::new(|_: NonNull<WKFindResult>| {});
    let query = NSString::from_str(&request.query);
    // SAFETY: all arguments are live; WebKit copies the block.
    unsafe { view.findString_withConfiguration_completionHandler(&query, Some(&config), &done) };
    true
}

/// Clears the find selection.
pub fn find_clear(webview: &PlatformWebview) {
    if let Ok((view, mtm)) = view(webview) {
        run_in_world(&view, mtm, CLEAR_SELECTION_SCRIPT, Box::new(|_| {}));
    }
}

/// Runs a script in the private world and passes a numeric result to `done`.
fn run_in_world(
    view: &WKWebView,
    mtm: MainThreadMarker,
    script: &str,
    done: Box<dyn Fn(Option<u32>)>,
) {
    let world = world(mtm);
    let block = RcBlock::new(move |result: *mut AnyObject, _error: *mut NSError| {
        // SAFETY: WebKit passes a valid object or null; `retain` handles null.
        let object = unsafe { Retained::retain(result) };
        let count = object
            .and_then(|o| o.downcast::<NSNumber>().ok())
            .and_then(|n| u32::try_from(n.integerValue()).ok());
        done(count);
    });
    let source = NSString::from_str(script);
    // SAFETY: all arguments are live; a `None` frame means the main frame; WebKit copies the
    // block and calls it once on the main thread.
    unsafe {
        view.evaluateJavaScript_inFrame_inContentWorld_completionHandler(
            &source,
            None,
            &world,
            Some(&block),
        );
    }
}

/// Turns off the fraud-check lookups, which call Apple's servers with page URLs.
///
/// # Errors
///
/// Fails when the engine handle is missing or the engine refuses the call.
pub fn harden(webview: &PlatformWebview) -> Result<(), String> {
    let (view, _) = view(webview)?;
    // SAFETY: plain getter on a live view.
    let config = unsafe { view.configuration() };
    // SAFETY: plain getter on a live configuration.
    let preferences = unsafe { config.preferences() };
    // SAFETY: setter on live preferences.
    unsafe { preferences.setFraudulentWebsiteWarningEnabled(false) };
    Ok(())
}

/// The `NSWindow` of a Tauri webview.
fn ns_window(webview: &PlatformWebview) -> Option<Retained<NSWindow>> {
    MainThreadMarker::new()?;
    let ptr = webview.ns_window().cast::<NSWindow>();
    // SAFETY: `PlatformWebview::ns_window` is the live window of this webview (Tauri keeps it
    // alive while `with_webview` runs), and we are on the main thread.
    unsafe { Retained::retain(ptr) }
}

/// The superview of `view`.
fn parent(view: &NSView) -> Option<Retained<NSView>> {
    // SAFETY: a plain getter on a live view on the main thread.
    unsafe { view.superview() }
}

/// Moves the traffic lights so their center is `center_y` points below the window top.
#[must_use]
pub fn place_window_buttons(webview: &PlatformWebview, center_y: f64) -> Option<WindowButtons> {
    let window = ns_window(webview)?;
    let kinds = [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ];
    let buttons: Vec<Retained<NSButton>> = kinds
        .iter()
        .filter_map(|k| window.standardWindowButton(*k))
        .collect();
    let (first, last) = (buttons.first()?, buttons.last()?);
    grow_title_bar(&window, first, center_y);
    for button in &buttons {
        center_button(&window, button, center_y);
    }
    Some(measure(&window, first, last))
}

/// Makes the title bar container tall enough to hold buttons centered at `center_y`.
fn grow_title_bar(window: &NSWindow, button: &NSButton, center_y: f64) {
    let Some(container) = parent(button).and_then(|bar| parent(&bar)) else {
        return;
    };
    let mut frame = container.frame();
    frame.size.height = (center_y * 2.0).max(frame.size.height);
    frame.origin.y = window.frame().size.height - frame.size.height;
    container.setFrame(frame);
}

/// Moves one button vertically; its left edge stays.
fn center_button(window: &NSWindow, button: &NSButton, center_y: f64) {
    let Some(bar) = parent(button) else {
        return;
    };
    let size = button.frame().size;
    let top = center_y - size.height / 2.0;
    let mut target = button.convertRect_toView(button.bounds(), None);
    target.origin.y = window.frame().size.height - top - size.height;
    let local = bar.convertRect_fromView(target, None);
    let mut origin = button.frame().origin;
    origin.y = local.origin.y;
    button.setFrameOrigin(origin);
}

/// The frames of the first and last button, in top-left window points.
fn measure(window: &NSWindow, first: &NSButton, last: &NSButton) -> WindowButtons {
    let height = window.frame().size.height;
    let a: NSRect = first.convertRect_toView(first.bounds(), None);
    let b: NSRect = last.convertRect_toView(last.bounds(), None);
    WindowButtons {
        left: a.origin.x,
        right: b.origin.x + b.size.width,
        center_y: height - (a.origin.y + a.size.height / 2.0),
    }
}

/// The ivars of [`FailRelay`].
pub struct FailIvars {
    /// wry's navigation delegate: every call but the two fail calls goes straight to it.
    inner: Retained<ProtocolObject<dyn WKNavigationDelegate>>,
    callback: Box<dyn Fn(LoadFailure)>,
}

define_class!(
    // SAFETY: NSObject has no subclassing rules; the class adds no Drop impl and only
    // reads its ivars on the main thread.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = FailIvars]
    struct FailRelay;

    // SAFETY: NSObjectProtocol has no required methods.
    unsafe impl NSObjectProtocol for FailRelay {}

    // SAFETY: the method signatures match the two optional fail methods of the protocol.
    unsafe impl WKNavigationDelegate for FailRelay {
        #[unsafe(method(webView:didFailProvisionalNavigation:withError:))]
        fn did_fail_provisional(
            &self,
            view: &WKWebView,
            navigation: Option<&WKNavigation>,
            error: &NSError,
        ) {
            (self.ivars().callback)(load_failure(error));
            let inner = &self.ivars().inner;
            if inner.respondsToSelector(sel!(webView:didFailProvisionalNavigation:withError:)) {
                // SAFETY: wry's delegate implements this method (checked above); all
                // arguments are the live ones WebKit passed in.
                unsafe { inner.webView_didFailProvisionalNavigation_withError(view, navigation, error) };
            }
        }

        #[unsafe(method(webView:didFailNavigation:withError:))]
        fn did_fail(&self, view: &WKWebView, navigation: Option<&WKNavigation>, error: &NSError) {
            (self.ivars().callback)(load_failure(error));
            let inner = &self.ivars().inner;
            if inner.respondsToSelector(sel!(webView:didFailNavigation:withError:)) {
                // SAFETY: as above.
                unsafe { inner.webView_didFailNavigation_withError(view, navigation, error) };
            }
        }
    }

    impl FailRelay {
        /// `WebKit` asks once, when the delegate is set, which methods exist: ours and wry's.
        #[unsafe(method(respondsToSelector:))]
        fn responds_to_selector(&self, selector: Sel) -> bool {
            // SAFETY: `respondsToSelector:` of NSObject takes a selector and returns a BOOL.
            let own: bool = unsafe { msg_send![super(self), respondsToSelector: selector] };
            own || self.ivars().inner.respondsToSelector(selector)
        }

        /// Every other delegate call goes to wry's delegate.
        #[unsafe(method(forwardingTargetForSelector:))]
        fn forwarding_target(&self, _selector: Sel) -> *mut AnyObject {
            let inner: &AnyObject = self.ivars().inner.as_ref();
            std::ptr::from_ref(inner).cast_mut()
        }
    }
);

impl FailRelay {
    fn new(
        inner: Retained<ProtocolObject<dyn WKNavigationDelegate>>,
        callback: Box<dyn Fn(LoadFailure)>,
        mtm: MainThreadMarker,
    ) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(FailIvars { inner, callback });
        // SAFETY: `init` of NSObject on a freshly allocated instance with ivars set.
        unsafe { msg_send![super(this), init] }
    }
}

/// The key under which a view keeps its [`FailRelay`] alive (only its address counts).
static RELAY_KEY: u8 = 0;

/// The failure that `WebKit` reports in `error`.
fn load_failure(error: &NSError) -> LoadFailure {
    let key = NSString::from_str("NSErrorFailingURLKey");
    let url = error
        .userInfo()
        .objectForKey(&key)
        .and_then(|value| value.downcast::<NSURL>().ok())
        .and_then(|url| url.absoluteString())
        .map(|text| text.to_string());
    LoadFailure {
        domain: error.domain().to_string(),
        code: i64::try_from(error.code()).unwrap_or_default(),
        url,
    }
}

/// Puts a [`FailRelay`] in front of wry's navigation delegate. The view keeps the relay alive.
///
/// # Errors
///
/// Fails when the engine handle or wry's delegate is missing.
pub fn on_load_failed(
    webview: &PlatformWebview,
    callback: Box<dyn Fn(LoadFailure)>,
) -> Result<(), String> {
    let (view, mtm) = view(webview)?;
    // SAFETY: a plain getter on a live view on the main thread.
    let inner = unsafe { view.navigationDelegate() }.ok_or("no navigation delegate")?;
    let relay = FailRelay::new(inner, callback, mtm);
    let object: &AnyObject = view.as_ref();
    let value: &AnyObject = relay.as_ref();
    // SAFETY: both objects are live; the view retains the relay for its whole life, and the
    // key is the address of a static.
    unsafe {
        objc2::ffi::objc_setAssociatedObject(
            std::ptr::from_ref(object).cast_mut(),
            std::ptr::from_ref(&RELAY_KEY).cast(),
            std::ptr::from_ref(value).cast_mut(),
            objc2::ffi::OBJC_ASSOCIATION_RETAIN_NONATOMIC,
        );
    }
    // SAFETY: a setter on a live view on the main thread; the delegate is weak, and the
    // association above keeps the relay alive.
    unsafe { view.setNavigationDelegate(Some(ProtocolObject::from_ref(&*relay))) };
    Ok(())
}
