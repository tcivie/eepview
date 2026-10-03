//! Router status changes: no `tab-*` webview exists unless the router is verified.

use super::{Core, Effect, Event, WebOp};
use crate::net::verify::Verdict;
use crate::types::RouterStatus;

/// The contract status of a VERIFY verdict. `gate_ok` is false when the gatekeeper could not
/// start; the router then counts as down.
#[must_use]
pub fn status_of(verdict: &Verdict, proxy: &str, gate_ok: bool) -> RouterStatus {
    let (state, detail) = match verdict {
        Verdict::Ok(_) if gate_ok => ("ok", None),
        Verdict::Ok(_) => (
            "down",
            Some("the gatekeeper proxy could not start".to_owned()),
        ),
        Verdict::Down(e) => ("down", Some(e.clone())),
        Verdict::NotI2p(e) => ("not-i2p", Some(e.clone())),
        Verdict::Outproxy(e) => ("outproxy", Some(e.clone())),
    };
    RouterStatus {
        state,
        proxy: proxy.to_owned(),
        version: None,
        detail,
    }
}

impl Core {
    /// A new VERIFY result. Going down destroys every tab webview (the tabs then show
    /// `eepview://router-down`); coming back loads the active tab again.
    pub fn router_changed(&mut self, status: RouterStatus) -> Vec<Effect> {
        let was_ok = self.router.is_ok();
        let now_ok = status.is_ok();
        self.router = status;
        let mut fx = vec![Effect::Emit(Event::Router)];
        if was_ok && !now_ok {
            fx.extend(self.destroy_all());
        }
        if now_ok && !was_ok {
            let id = self.tabs.active_id();
            fx.extend(self.ensure_loaded(id));
            fx.extend([Effect::Emit(Event::TabsChanged), Effect::Layout]);
        }
        fx
    }

    fn destroy_all(&mut self) -> Vec<Effect> {
        let mut fx = Vec::new();
        for tab in self.tabs.iter_mut().filter(|t| t.web_js.is_some()) {
            tab.web_js = None;
            tab.web_url = None;
            tab.loading = false;
            tab.session.engine_reset();
            fx.push(Effect::Web(WebOp::Destroy(tab.id)));
        }
        self.find = None;
        fx.extend([Effect::Emit(Event::TabsChanged), Effect::Layout]);
        fx
    }
}
