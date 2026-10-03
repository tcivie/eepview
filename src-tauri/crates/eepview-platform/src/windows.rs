//! `WebView2` calls through webview2-com. Every `unsafe` block holds one COM call and names
//! why it is sound.

use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL, ICoreWebView2, ICoreWebView2_12, ICoreWebView2Settings4,
    ICoreWebView2Settings8,
};
use webview2_com::{
    StatusBarTextChangedEventHandler, WebResourceRequestedEventHandler, take_pwstr,
};
use windows_core::{Interface, PWSTR, w};

use crate::{Nav, PlatformWebview};

fn core(webview: &PlatformWebview) -> Result<ICoreWebView2, String> {
    let controller = webview.controller();
    // SAFETY: the controller is live while `with_webview` runs; COM call on the UI thread.
    unsafe { controller.CoreWebView2() }.map_err(|e| e.to_string())
}

/// Answers 403 to every request whose URL `allow` refuses (the second L3 belt).
pub fn attach_rules(webview: &PlatformWebview, allow: fn(&str) -> bool) -> Result<(), String> {
    let core = core(webview)?;
    let env = webview.environment();
    // SAFETY: COM call on a live object with a static wide string.
    unsafe { core.AddWebResourceRequestedFilter(w!("*"), COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL) }
        .map_err(|e| e.to_string())?;
    let handler = WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else { return Ok(()) };
        // SAFETY: getter on the live event args.
        let request = unsafe { args.Request() }?;
        let mut uri = PWSTR::null();
        // SAFETY: getter on the live request; WebView2 allocates the string we take.
        unsafe { request.Uri(&mut uri) }?;
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
        unsafe { source.StatusBarText(&mut text) }?;
        callback(crate::hover_target(&take_pwstr(text)));
        Ok(())
    }));
    let mut token = 0_i64;
    // SAFETY: COM call on a live object; the token outlives the call.
    unsafe { core12.add_StatusBarTextChanged(&handler, &raw mut token) }.map_err(|e| e.to_string())
}

/// One step on the engine's navigation list.
pub fn go(webview: &PlatformWebview, nav: Nav) -> Result<(), String> {
    let core = core(webview)?;
    let result = match nav {
        // SAFETY: plain COM navigation calls on a live object on the UI thread.
        Nav::Back => unsafe { core.GoBack() },
        // SAFETY: as above.
        Nav::Forward => unsafe { core.GoForward() },
        // SAFETY: as above.
        Nav::Reload | Nav::HardReload => unsafe { core.Reload() },
        // SAFETY: as above.
        Nav::Stop => unsafe { core.Stop() },
    };
    result.map_err(|e| e.to_string())
}

/// Autofill, password saving and `SmartScreen` reputation checks off.
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
