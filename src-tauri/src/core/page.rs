// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Events from the engines: navigation requests, page loads, titles, new windows, hover.

use tauri::Url;

use super::{Core, Effect, EngineOp, Event, WebOp};
use crate::hover;
use crate::nav::{guard, host_of, internal_from_file, internal_with, is_allowed, is_web};
use crate::tabs::Place;

/// Titles of the error pages of the I2P router proxy.
const ERROR_TITLES: [&str; 2] = ["Website Unreachable", "Website Not Found In Addressbook"];

/// True for the title of a router proxy error page.
fn is_error_title(title: &str) -> bool {
    let title = title.trim();
    ERROR_TITLES.iter().any(|e| e.eq_ignore_ascii_case(title))
}

impl Core {
    /// `on_navigation` of a `tab-*` webview. Returns whether the engine may go on (layer L4).
    ///
    /// The engines report main-frame and sub-frame navigations alike, with no frame flag. So a
    /// refused navigation replaces the tab with the blocked page only when it is the link
    /// under the mouse (a click). Anything else (a frame, a script, a redirect) gets a toast.
    pub fn tab_navigation(&mut self, id: u32, url: &Url) -> (bool, Vec<Effect>) {
        if guard(url) {
            return (true, Vec::new());
        }
        let shown = hover::link(url.as_str()).map_or_else(String::new, |t| t.text);
        let clicked = self.pointed.as_ref() == Some(url);
        let top_level = clicked && matches!(url.scheme(), "http" | "https" | "file" | "ftp");
        if top_level {
            self.pointed = None;
            let page = internal_with("blocked", &[("url", url.as_str())]);
            return (false, self.go_internal(id, &page));
        }
        (false, vec![Self::toast("warn", shown)])
    }

    /// The main frame of a tab started to show `url`. Ignored while the tab shows an internal
    /// page: the hidden web view may still run a timer or a refresh.
    pub fn page_started(&mut self, id: u32, url: &str) -> Vec<Effect> {
        if !self.shows_web(id) || !is_allowed(url) {
            // Only an I2P address may enter tab state, so it can never be replayed.
            return Vec::new();
        }
        let host = host_of(url).unwrap_or_default();
        let js = self.js_of(&host);
        let zoom = self.zoom_of(&host);
        let active = id == self.tabs.active_id();
        let Some(tab) = self.tabs.get_mut(id) else {
            return Vec::new();
        };
        if tab.web_js.is_some_and(|live| live != js) {
            // A host with another JS choice: build the webview again for it.
            return self.go_web(id, url);
        }
        tab.loading = true;
        tab.failed = false;
        url.clone_into(&mut tab.url);
        tab.web_url = Some(url.to_owned());
        tab.session.commit_web(url);
        let mut fx = vec![
            Effect::Web(WebOp::Engine(id, EngineOp::Zoom(zoom))),
            Effect::Emit(Event::TabUpdated(id)),
            Effect::Layout,
        ];
        if active {
            fx.extend(self.show_hover(hover::loading(url)));
            fx.extend(self.take_focus(id));
        }
        fx
    }

    /// The first commit of an address-bar navigation takes keyboard focus, once, whatever
    /// address it commits (a redirect target counts).
    fn take_focus(&mut self, id: u32) -> Option<Effect> {
        (self.focus_on_commit == Some(id)).then(|| {
            self.focus_on_commit = None;
            Effect::FocusContent
        })
    }

    /// The main frame of a tab finished loading `url`.
    pub fn page_finished(&mut self, id: u32, url: &str, now: u64) -> Vec<Effect> {
        if !self.shows_web(id) {
            return Vec::new();
        }
        let record = self.settings.history.enabled;
        let Some(tab) = self.tabs.get_mut(id) else {
            return Vec::new();
        };
        if !is_allowed(url) {
            return Vec::new();
        }
        tab.loading = false;
        tab.session.commit_web(url);
        let title = tab.web_title.clone();
        // A failed load is shown, but it is not a visit worth remembering.
        let record = record && !tab.failed && !is_error_title(&title);
        let mut fx = vec![Effect::Emit(Event::TabUpdated(id))];
        if record {
            self.history.visit(url, &title, now);
            fx.extend(self.save_history());
        }
        fx.extend(self.hover_out());
        fx
    }

    /// The main frame of a tab loaded `url`, but the load failed: a router or proxy error page,
    /// a 5xx answer, or a gatekeeper refusal. The tab stops loading. No history entry is made.
    pub fn page_failed(&mut self, id: u32, url: &str) -> Vec<Effect> {
        if !self.shows_web(id) || !is_allowed(url) {
            return Vec::new();
        }
        let Some(tab) = self.tabs.get_mut(id) else {
            return Vec::new();
        };
        tab.loading = false;
        tab.failed = true;
        let mut fx = vec![Effect::Emit(Event::TabUpdated(id))];
        fx.extend(self.hover_out());
        fx
    }

    /// The document title of a tab changed.
    pub fn title_changed(&mut self, id: u32, title: &str) -> Vec<Effect> {
        if !self.shows_web(id) {
            return Vec::new();
        }
        let Some(tab) = self.tabs.get_mut(id) else {
            return Vec::new();
        };
        title.clone_into(&mut tab.web_title);
        let Some(url) = tab.web_url.clone().filter(|u| *u == tab.url) else {
            return Vec::new();
        };
        title.clone_into(&mut tab.title);
        // While the page loads, the finish event records the title; a failed page never
        // renames the entry of an address.
        let settled = !tab.loading && !tab.failed && !is_error_title(title);
        let mut fx = vec![Effect::Emit(Event::TabUpdated(id))];
        if settled && self.settings.history.enabled && self.history.set_title(&url, title) {
            fx.extend(self.save_history());
        }
        fx
    }

    /// A page asked for a new window (`target=_blank`, `window.open`): a new tab, same guard.
    pub fn new_window(&mut self, url: &Url) -> Vec<Effect> {
        if !guard(url) || url.as_str().starts_with("about:") {
            let shown = hover::link(url.as_str()).map_or_else(String::new, |t| t.text);
            return vec![Self::toast("warn", shown)];
        }
        self.tab_new(Some(url.as_str()), Place::AfterActive).1
    }

    /// A page tried to download a file. Downloads are refused for now.
    #[must_use]
    pub fn download_refused(url: &str) -> Vec<Effect> {
        vec![Self::toast(
            "info",
            format!("Downloads are off in this version: {url}"),
        )]
    }

    /// The `internal` webview finished loading a bundled page (it may follow its own links).
    pub fn internal_loaded(&mut self, url: &Url) -> Vec<Effect> {
        let Some(page) = internal_from_file(url) else {
            return Vec::new();
        };
        let id = self.tabs.active_id();
        match self.tabs.get(id) {
            Some(tab) if !is_web(&tab.url) && tab.url != page => self.go_internal(id, &page),
            _ => Vec::new(),
        }
    }

    fn shows_web(&self, id: u32) -> bool {
        self.tabs.get(id).is_some_and(|t| is_web(&t.url))
    }

    /// The mouse moved over a link (`Some`) or off it (`None`) in tab `id`.
    pub fn hover_link(&mut self, id: u32, target: Option<&str>) -> Vec<Effect> {
        if id != self.tabs.active_id() {
            return Vec::new();
        }
        self.pointed = target.and_then(|t| Url::parse(t).ok());
        match target.and_then(hover::link).map(|t| self.hover.point(t)) {
            Some(hover::Step::Show(text)) => vec![Effect::Emit(Event::Hover(Some(text)))],
            Some(hover::Step::Wait(generation)) => vec![Effect::HoverLater(generation)],
            Some(hover::Step::Keep) => Vec::new(),
            None => self.hover_out(),
        }
    }

    fn show_hover(&mut self, text: Option<hover::HoverText>) -> Vec<Effect> {
        text.and_then(|t| self.hover.show_now(t))
            .map(|t| Effect::Emit(Event::Hover(Some(t))))
            .into_iter()
            .collect()
    }

    fn hover_out(&mut self) -> Vec<Effect> {
        if self.hover.leave() {
            vec![Effect::Emit(Event::Hover(None))]
        } else {
            Vec::new()
        }
    }

    /// The text the status bubble shows, if any.
    #[must_use]
    pub fn hover_text(&self) -> Option<&hover::HoverText> {
        self.hover.shown()
    }

    /// The show delay of generation `generation` passed.
    pub fn hover_expire(&mut self, generation: u64) -> Vec<Effect> {
        self.hover
            .expire(generation)
            .map(|t| Effect::Emit(Event::Hover(Some(t))))
            .into_iter()
            .collect()
    }
}
