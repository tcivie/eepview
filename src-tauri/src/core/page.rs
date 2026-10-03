// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Events from the engines: navigation requests, page loads, titles, new windows, hover.

use tauri::Url;

use super::{Core, Effect, EngineOp, Event, WebOp};
use crate::hover;
use crate::nav::{guard, host_of, internal_from_file, internal_with, is_web};
use crate::tabs::Place;

impl Core {
    /// `on_navigation` of a `tab-*` webview (main frame and sub-frames). Returns whether the
    /// engine may go on (layer L4).
    pub fn tab_navigation(&mut self, id: u32, url: &Url) -> (bool, Vec<Effect>) {
        if guard(url) {
            return (true, Vec::new());
        }
        let shown = hover::link(url.as_str()).map_or_else(String::new, |t| t.text);
        let idle = self.tabs.get(id).is_some_and(|t| !t.loading);
        let top_level = idle && matches!(url.scheme(), "http" | "https" | "file" | "ftp");
        if top_level {
            let page = internal_with("blocked", &[("url", url.as_str())]);
            return (false, self.go_internal(id, &page));
        }
        (false, vec![Self::toast("warn", shown)])
    }

    /// The main frame of a tab started to show `url`.
    pub fn page_started(&mut self, id: u32, url: &str) -> Vec<Effect> {
        let host = host_of(url).unwrap_or_default();
        let js = self.js_of(&host);
        let zoom = self.zoom_of(&host);
        let active = id == self.tabs.active_id();
        let Some(tab) = self.tabs.get_mut(id) else {
            return Vec::new();
        };
        if !is_web(url) {
            return Vec::new();
        }
        if tab.web_js.is_some_and(|live| live != js) {
            // A host with another JS choice: build the webview again for it.
            return self.go_web(id, url);
        }
        tab.loading = true;
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
        }
        fx
    }

    /// The main frame of a tab finished loading `url`.
    pub fn page_finished(&mut self, id: u32, url: &str, now: u64) -> Vec<Effect> {
        let record = self.settings.history.enabled;
        let Some(tab) = self.tabs.get_mut(id) else {
            return Vec::new();
        };
        if !is_web(url) {
            return Vec::new();
        }
        tab.loading = false;
        tab.session.commit_web(url);
        let title = tab.web_title.clone();
        let mut fx = vec![Effect::Emit(Event::TabUpdated(id))];
        if record {
            self.history.visit(url, &title, now);
            fx.extend(self.save_history());
        }
        fx.extend(self.hover_out());
        fx
    }

    /// The document title of a tab changed.
    pub fn title_changed(&mut self, id: u32, title: &str) -> Vec<Effect> {
        let Some(tab) = self.tabs.get_mut(id) else {
            return Vec::new();
        };
        title.clone_into(&mut tab.web_title);
        let Some(url) = tab.web_url.clone().filter(|u| *u == tab.url) else {
            return Vec::new();
        };
        title.clone_into(&mut tab.title);
        let mut fx = vec![Effect::Emit(Event::TabUpdated(id))];
        if self.settings.history.enabled && self.history.set_title(&url, title) {
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

    /// The mouse moved over a link (`Some`) or off it (`None`) in tab `id`.
    pub fn hover_link(&mut self, id: u32, target: Option<&str>) -> Vec<Effect> {
        if id != self.tabs.active_id() {
            return Vec::new();
        }
        match target.and_then(hover::link) {
            Some(text) => self.show_hover(Some(text)),
            None => self.hover_out(),
        }
    }

    fn show_hover(&mut self, text: Option<hover::HoverText>) -> Vec<Effect> {
        text.and_then(|t| self.hover.show(t))
            .map(|t| Effect::Emit(Event::Hover(Some(t))))
            .into_iter()
            .collect()
    }

    fn hover_out(&mut self) -> Vec<Effect> {
        if self.hover.shown().is_none() {
            return Vec::new();
        }
        vec![Effect::HoverLater(self.hover.leave())]
    }

    /// The hide delay of generation `generation` passed.
    pub fn hover_expire(&mut self, generation: u64) -> Vec<Effect> {
        if self.hover.expire(generation) {
            vec![Effect::Emit(Event::Hover(None))]
        } else {
            Vec::new()
        }
    }
}
