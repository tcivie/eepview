// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

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
    };
    RouterStatus {
        state,
        proxy: proxy.to_owned(),
        version: None,
        detail,
        paused: false,
        managed: false,
    }
}

impl Core {
    /// A new VERIFY result. Going down destroys every tab webview (the tabs then show
    /// `eepview://router-down`); coming back loads the active tab again.
    ///
    /// While paused, the new status is kept but counts as not ok: nothing loads.
    pub fn router_changed(&mut self, mut status: RouterStatus) -> Vec<Effect> {
        status.paused = self.router.paused;
        // VERIFY does not read the version: keep the one the router statistics gave.
        status.version = self.router.version.clone().filter(|_| status.state == "ok");
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

    /// The router reported its version in its statistics. Tells the UI when it changes.
    pub fn router_version(&mut self, version: Option<&str>) -> Vec<Effect> {
        match version {
            Some(v) if self.router.version.as_deref() != Some(v) => {
                self.router.version = Some(v.to_owned());
                vec![Effect::Emit(Event::Router)]
            }
            _ => Vec::new(),
        }
    }

    /// True while the user has paused the connection.
    #[must_use]
    pub fn paused(&self) -> bool {
        self.router.paused
    }

    /// `connection_pause()`: every tab webview goes; tabs show the router-down page with
    /// `reason=paused`. The shell closes the gatekeeper after this.
    pub fn pause(&mut self) -> Vec<Effect> {
        if self.router.paused {
            return Vec::new();
        }
        self.router.paused = true;
        let mut fx = vec![Effect::Emit(Event::Router)];
        fx.extend(self.destroy_all());
        fx
    }

    /// `connection_resume()`: back to `verifying`. The next VERIFY that passes opens the
    /// gatekeeper and loads the active tab again; one that fails keeps it closed.
    pub fn resume(&mut self) -> Vec<Effect> {
        if !self.router.paused {
            return Vec::new();
        }
        self.router.paused = false;
        self.router.state = "verifying";
        vec![
            Effect::Emit(Event::Router),
            Effect::Emit(Event::TabsChanged),
            Effect::Layout,
        ]
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
