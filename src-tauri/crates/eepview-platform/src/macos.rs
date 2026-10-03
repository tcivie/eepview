// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! `WKWebView` calls (macOS 14+). Every `unsafe` block holds one call into Objective-C and
//! names why it is sound.

use std::cell::{Cell, RefCell};
use std::ptr::NonNull;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadOnly, Message, define_class, msg_send};
use objc2_foundation::{MainThreadMarker, NSError, NSNumber, NSObjectProtocol, NSString};
use objc2_web_kit::{
    WKContentRuleList, WKContentRuleListStore, WKContentWorld, WKFindConfiguration, WKFindResult,
    WKScriptMessage, WKScriptMessageHandler, WKUserContentController, WKUserScript,
    WKUserScriptInjectionTime, WKWebView,
};

use objc2_app_kit::{NSButton, NSView, NSWindow, NSWindowButton};
use objc2_foundation::NSRect;

use crate::{
    CLEAR_SELECTION_SCRIPT, FindRequest, HOVER_CHANNEL, HOVER_SCRIPT, Nav, PlatformWebview, Rules,
    WindowButtons, count_script, hover_target,
};

/// The identifier of the compiled rule list in the default store.
const RULES_ID: &str = "eepview-i2p-only";

thread_local! {
    /// The compiled rule list, shared by every content webview (main thread only).
    static RULES: RefCell<Option<Retained<WKContentRuleList>>> = const { RefCell::new(None) };
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
    let cached = RULES.with(|r| r.borrow().clone());
    if let Some(list) = cached {
        add_rules(&view, &list);
        return done(Ok(()));
    }
    compile(&view, rules.json, mtm, done);
}

fn add_rules(view: &WKWebView, list: &WKContentRuleList) {
    let controller = content_controller(view);
    // SAFETY: both objects are live; WebKit retains the list.
    unsafe { controller.addContentRuleList(list) };
}

fn compile(
    view: &WKWebView,
    json: &str,
    mtm: MainThreadMarker,
    done: Box<dyn FnOnce(Result<(), String>)>,
) {
    // SAFETY: `defaultStore` is a class getter; main thread per `mtm`.
    let Some(store) = (unsafe { WKContentRuleListStore::defaultStore(mtm) }) else {
        return done(Err("no content rule list store".into()));
    };
    let view = view.retain();
    let done = Cell::new(Some(done));
    let block = RcBlock::new(move |list: *mut WKContentRuleList, error: *mut NSError| {
        let Some(done) = done.take() else { return };
        // SAFETY: WebKit passes a valid list or null; `retain` handles null.
        match unsafe { Retained::retain(list) } {
            Some(list) => {
                add_rules(&view, &list);
                RULES.with(|r| *r.borrow_mut() = Some(list));
                done(Ok(()));
            }
            None => done(Err(error_text(error))),
        }
    });
    let id = NSString::from_str(RULES_ID);
    let rules = NSString::from_str(json);
    // SAFETY: all arguments are live for the call; WebKit copies the block and calls it once
    // on the main thread.
    unsafe {
        store.compileContentRuleListForIdentifier_encodedContentRuleList_completionHandler(
            Some(&id),
            Some(&rules),
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
    let source = NSString::from_str(HOVER_SCRIPT);
    // SAFETY: designated initializer on a fresh allocation; all arguments are live.
    let script = unsafe {
        WKUserScript::initWithSource_injectionTime_forMainFrameOnly_inContentWorld(
            mtm.alloc(),
            &source,
            WKUserScriptInjectionTime::AtDocumentStart,
            false,
            &world,
        )
    };
    // SAFETY: both objects are live; the controller retains the script.
    unsafe { controller.addUserScript(&script) };
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
