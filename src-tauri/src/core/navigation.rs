// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Address-bar navigation, back/forward, reload and stop.

use super::{Core, Effect, EngineOp, Event, Load, WebOp, internal_title};
use crate::nav::{Target, classify, host_of, internal_with, is_allowed, is_web};
use crate::session::{Step, Traverse};
use crate::types::NavResult;

impl Core {
    /// `navigate(input)` on the active tab.
    pub fn navigate(&mut self, input: &str) -> (NavResult, Vec<Effect>) {
        if input.trim().is_empty() {
            // Blank input is ignored (owner decision).
            return (NavResult::ok(), Vec::new());
        }
        let id = self.tabs.active_id();
        let target = classify(input);
        let result = match &target {
            Target::Refused(r) => NavResult::refused(r.as_str()),
            Target::Web(_) if !self.router.is_ok() => NavResult::refused("router-down"),
            _ => NavResult::ok(),
        };
        let mut fx = self.open_target(id, &target, input);
        fx.extend(self.focus_plan(id, matches!(target, Target::Web(_))));
        (result, fx)
    }

    /// Keyboard focus after an address-bar navigation: at once for an internal page (or while
    /// the router is down), on the first commit for a web page.
    fn focus_plan(&mut self, id: u32, web: bool) -> Option<Effect> {
        self.focus_on_commit = None;
        if web && self.router.is_ok() {
            self.focus_on_commit = Some(id);
            return None;
        }
        Some(Effect::FocusContent)
    }

    /// The `eepview://` or `http://` string a target lands on.
    pub(super) fn landing(target: &Target, raw: &str) -> String {
        match target {
            Target::Web(url) => url.to_string(),
            Target::Internal(page) => page.clone(),
            Target::Search(q) => internal_with("history", &[("q", q)]),
            Target::Refused(_) => internal_with("blocked", &[("url", raw.trim())]),
        }
    }

    pub(super) fn open_target(&mut self, id: u32, target: &Target, raw: &str) -> Vec<Effect> {
        let url = Self::landing(target, raw);
        if matches!(target, Target::Web(_)) {
            self.go_web(id, &url)
        } else {
            self.go_internal(id, &url)
        }
    }

    pub(super) fn go_web(&mut self, id: u32, url: &str) -> Vec<Effect> {
        let ok = self.router.is_ok();
        let Some(tab) = self.tabs.get_mut(id) else {
            return Vec::new();
        };
        url.clone_into(&mut tab.url);
        tab.title = String::new();
        tab.loading = ok;
        let mut fx = self.load(id, url);
        fx.extend([Effect::Emit(Event::TabUpdated(id)), Effect::Layout]);
        fx
    }

    pub(super) fn go_internal(&mut self, id: u32, page: &str) -> Vec<Effect> {
        let Some(tab) = self.tabs.get_mut(id) else {
            return Vec::new();
        };
        let mut fx = Vec::new();
        if tab.loading && tab.web_js.is_some() {
            fx.push(Effect::Web(WebOp::Engine(id, EngineOp::Stop)));
        }
        tab.session.push_internal(page);
        page.clone_into(&mut tab.url);
        tab.title = internal_title(page);
        tab.loading = false;
        fx.extend([Effect::Emit(Event::TabUpdated(id)), Effect::Layout]);
        fx
    }

    /// Loads an allowed URL in the tab webview, building it when needed. Nothing happens
    /// while the router is not verified: no `tab-*` webview exists then.
    ///
    /// This is the one place a `WebOp::Load` is made: a URL that fails the I2P rule never
    /// reaches an engine, whatever path brought it here.
    pub(super) fn load(&mut self, id: u32, url: &str) -> Vec<Effect> {
        if !is_allowed(url) {
            let page = internal_with("blocked", &[("url", url)]);
            return self.go_internal(id, &page);
        }
        if !self.router.is_ok() {
            return Vec::new();
        }
        let host = host_of(url).unwrap_or_default();
        let (js, zoom) = (self.js_of(&host), self.zoom_of(&host));
        let private = !self.settings.keep_cookies;
        let Some(tab) = self.tabs.get_mut(id) else {
            return Vec::new();
        };
        let rebuild = tab.web_js.is_some_and(|live| live != js);
        if rebuild || tab.web_js.is_none() {
            tab.session.engine_reset();
        }
        tab.web_js = Some(js);
        tab.loading = true;
        let load = Load {
            tab: id,
            url: url.to_owned(),
            js,
            rebuild,
            private,
            zoom,
        };
        vec![Effect::Web(WebOp::Load(load))]
    }

    /// Builds the webview of a web tab that has none (lazy tabs, router back up).
    pub(super) fn ensure_loaded(&mut self, id: u32) -> Vec<Effect> {
        let Some(tab) = self.tabs.get(id) else {
            return Vec::new();
        };
        if !is_web(&tab.url) || tab.web_js.is_some() {
            return Vec::new();
        }
        let url = tab.url.clone();
        self.load(id, &url)
    }

    /// `go_back` / `go_forward` on the active tab.
    pub fn step(&mut self, step: Step) -> Vec<Effect> {
        let id = self.tabs.active_id();
        let Some(tab) = self.tabs.get_mut(id) else {
            return Vec::new();
        };
        let live = tab.web_js.and(tab.web_url.clone());
        match tab.session.step(step, live.as_deref()) {
            None => Vec::new(),
            Some(Traverse::Internal(url)) => {
                tab.url.clone_from(&url);
                tab.title = internal_title(&url);
                vec![Effect::Emit(Event::TabUpdated(id)), Effect::Layout]
            }
            Some(Traverse::Reveal(url)) => {
                tab.url = url;
                tab.title.clone_from(&tab.web_title);
                vec![Effect::Emit(Event::TabUpdated(id)), Effect::Layout]
            }
            Some(Traverse::Engine(s)) => Self::engine_step(tab, s),
            Some(Traverse::Load(url)) => self.go_web(id, &url),
        }
    }

    fn engine_step(tab: &mut crate::tabs::Tab, step: Step) -> Vec<Effect> {
        tab.loading = true;
        let op = match step {
            Step::Back => EngineOp::Back,
            Step::Forward => EngineOp::Forward,
        };
        vec![
            Effect::Web(WebOp::Engine(tab.id, op)),
            Effect::Emit(Event::TabUpdated(tab.id)),
        ]
    }

    /// `reload(hard)` on the active tab.
    pub fn reload(&mut self, hard: bool) -> Vec<Effect> {
        let id = self.tabs.active_id();
        let Some(tab) = self.tabs.get_mut(id) else {
            return Vec::new();
        };
        if !is_web(&tab.url) {
            return vec![Effect::Layout];
        }
        if tab.web_js.is_none() {
            return self.ensure_loaded(id);
        }
        tab.loading = true;
        let op = if hard {
            EngineOp::HardReload
        } else {
            EngineOp::Reload
        };
        vec![
            Effect::Web(WebOp::Engine(id, op)),
            Effect::Emit(Event::TabUpdated(id)),
        ]
    }

    /// `stop()` on the active tab.
    pub fn stop(&mut self) -> Vec<Effect> {
        let id = self.tabs.active_id();
        let Some(tab) = self.tabs.get_mut(id) else {
            return Vec::new();
        };
        if tab.web_js.is_none() || !tab.loading {
            return Vec::new();
        }
        tab.loading = false;
        vec![
            Effect::Web(WebOp::Engine(id, EngineOp::Stop)),
            Effect::Emit(Event::TabUpdated(id)),
        ]
    }

    /// `home()` on the active tab.
    pub fn home(&mut self) -> Vec<Effect> {
        let home = self.home_url();
        self.navigate(&home).1
    }
}
