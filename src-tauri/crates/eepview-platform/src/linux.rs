//! `WebKitGTK` calls. The webkit2gtk crate wraps them safely, so this module has no `unsafe`.

use std::cell::Cell;
use std::rc::Rc;

use webkit2gtk::glib::ObjectExt;
use webkit2gtk::{FindControllerExt, FindOptions, HitTestResultExt, SettingsExt, WebViewExt};

use crate::{FindRequest, Nav, PlatformWebview, Rules};

/// No request filter: webkit2gtk has no binding for one, and the engine makes no requests
/// of its own (spike S1). The CSP of the gatekeeper is the L3 layer here.
pub fn attach_rules(
    _webview: &PlatformWebview,
    _rules: Rules<'_>,
    done: Box<dyn FnOnce(Result<(), String>)>,
) {
    done(Ok(()));
}

/// The link under the mouse, from `mouse-target-changed`.
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
pub fn harden(webview: &PlatformWebview) -> Result<(), String> {
    let settings = WebViewExt::settings(&webview.inner()).ok_or("no WebKit settings")?;
    settings.set_enable_webrtc(false);
    settings.set_enable_media_stream(false);
    Ok(())
}
