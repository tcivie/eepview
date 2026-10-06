// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Router status changes: no `tab-*` webview exists unless the router is verified.

use super::checks::{CheckFacts, outproxy_outcome, tunnels_outcome, version_outcome};
use super::{Core, Effect, Event, WebOp};
use crate::net::verify::Verdict;
use crate::types::{CheckId, RouterReport, RouterStatus, VerifyCheck};

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

    /// The four router checks, in contract order (V1 of `docs/wiki/router-checks.md`).
    #[must_use]
    pub fn checks(&self) -> &[VerifyCheck] {
        self.checks.list()
    }

    /// The `router_status()` answer and the `router-status` payload: status and checks.
    #[must_use]
    pub fn router_report(&self) -> RouterReport {
        RouterReport {
            status: self.router.clone(),
            checks: self.checks.list().to_vec(),
        }
    }

    /// V5: a VERIFY round starts. A `pending` check 1 turns `running`. Nothing while paused.
    pub fn verify_started(&mut self) -> Vec<Effect> {
        if self.router.paused || !self.checks.start_round() {
            return Vec::new();
        }
        vec![Effect::Emit(Event::Router)]
    }

    /// The result of a VERIFY round at `now` (Unix ms): [`Core::router_changed`], then the
    /// checks (V6, V7, V8, V9).
    pub fn router_checked(&mut self, now: u64, status: RouterStatus) -> Vec<Effect> {
        let fx = self.router_changed(status);
        if self.router.paused {
            self.checks.reset();
        } else if self.router.state == "ok" {
            self.checks.round(now, None);
        } else {
            let reason = self.router.detail.clone();
            let reason = reason.unwrap_or_else(|| self.router.state.to_owned());
            self.checks.round(now, Some(reason));
        }
        fx
    }

    /// V9 to V14: checks 2 to 4 from the facts of a round at `now`. Nothing while the gate
    /// is closed or the connection is paused. Emits `router-status` only on a change.
    pub fn checks_seen(&mut self, now: u64, facts: &CheckFacts) -> Vec<Effect> {
        if !self.router.is_ok() || !self.checks.gate_passed() {
            return Vec::new();
        }
        let runs = [
            (
                CheckId::Version,
                version_outcome(facts.kind, facts.version.as_deref()),
            ),
            (CheckId::NoOutproxy, outproxy_outcome(&facts.outproxy)),
            (CheckId::Tunnels, tunnels_outcome(facts.stats.as_ref())),
        ];
        let mut changed = false;
        for (id, outcome) in runs {
            changed |= self.checks.set(id, now, outcome);
        }
        if changed {
            vec![Effect::Emit(Event::Router)]
        } else {
            Vec::new()
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
        self.checks.reset();
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
        self.checks.reset();
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
