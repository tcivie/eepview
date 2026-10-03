//! Find in page, zoom and the per-site JavaScript switch.

use super::{Core, Effect, EngineOp, Event, FindOp, FindState, WebOp};
use crate::nav::host_of;
use crate::store::site_prefs::zoom_step;
use crate::types::FindResult;

/// How zoom moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zoom {
    /// One step up.
    In,
    /// One step down.
    Out,
    /// Back to the default zoom.
    Reset,
}

impl Core {
    /// `find(query, forward, matchCase)` on the active tab.
    pub fn find(&mut self, query: &str, forward: bool, match_case: bool) -> Vec<Effect> {
        self.find_open = true;
        let id = self.tabs.active_id();
        let live = self
            .tabs
            .get(id)
            .is_some_and(|t| t.web_js.is_some() && t.url.starts_with("http"));
        if query.is_empty() || !live {
            self.find = None;
            let none = FindResult {
                query: query.to_owned(),
                matches: None,
                active: None,
            };
            return vec![Effect::Emit(Event::Find(none)), Effect::Layout];
        }
        let fresh = self.start_find(id, query, match_case);
        let op = FindOp {
            query: query.to_owned(),
            forward,
            match_case,
            fresh,
        };
        let mut fx = vec![
            Effect::Web(WebOp::Engine(id, EngineOp::Find(op))),
            Effect::Layout,
        ];
        if !fresh {
            fx.extend(self.advance(forward));
        }
        fx
    }

    /// Starts a new search unless it is the same as the current one. True when new.
    fn start_find(&mut self, id: u32, query: &str, match_case: bool) -> bool {
        let fresh = self
            .find
            .as_ref()
            .is_none_or(|f| f.tab != id || f.query != query || f.match_case != match_case);
        if fresh {
            self.find = Some(FindState {
                tab: id,
                query: query.to_owned(),
                match_case,
                matches: None,
                active: None,
            });
        }
        fresh
    }

    fn advance(&mut self, forward: bool) -> Vec<Effect> {
        let Some(find) = self.find.as_mut() else {
            return Vec::new();
        };
        let Some(n) = find.matches.filter(|n| *n > 0) else {
            return Vec::new();
        };
        let current = find.active.unwrap_or(0);
        find.active = Some(if forward {
            current % n + 1
        } else {
            (current + n - 2) % n + 1
        });
        vec![Effect::Emit(Event::Find(Self::result_of(find)))]
    }

    fn result_of(find: &FindState) -> FindResult {
        FindResult {
            query: find.query.clone(),
            matches: find.matches,
            active: find.active,
        }
    }

    /// The engine counted the matches of the current search in tab `id` (`None`: no count).
    pub fn find_counted(&mut self, id: u32, matches: Option<u32>) -> Vec<Effect> {
        let Some(find) = self.find.as_mut().filter(|f| f.tab == id) else {
            return Vec::new();
        };
        find.matches = matches;
        find.active = match matches {
            Some(n) if n > 0 => Some(find.active.unwrap_or(1).min(n)),
            _ => None,
        };
        vec![Effect::Emit(Event::Find(Self::result_of(find)))]
    }

    /// `find_close()`.
    pub fn find_close(&mut self) -> Vec<Effect> {
        self.find_open = false;
        let mut fx = vec![Effect::Layout];
        if let Some(find) = self.find.take() {
            fx.push(Effect::Web(WebOp::Engine(find.tab, EngineOp::FindClear)));
        }
        fx
    }

    /// `zoom_in()`, `zoom_out()`, `zoom_reset()` on the active site; remembered per host.
    pub fn zoom(&mut self, how: Zoom) -> Vec<Effect> {
        let Some(host) = self.tabs.active().and_then(|t| host_of(&t.url)) else {
            return Vec::new();
        };
        let default = self.settings.zoom_default;
        let next = match how {
            Zoom::In => zoom_step(self.zoom_of(&host), true),
            Zoom::Out => zoom_step(self.zoom_of(&host), false),
            Zoom::Reset => default,
        };
        self.prefs.set_zoom(&host, next, default);
        let mut fx = self.save_prefs();
        let same_host = self
            .tabs
            .iter()
            .filter(|t| host_of(&t.url).as_deref() == Some(&host));
        for tab in same_host {
            let live = tab.web_js.is_some();
            fx.extend(live.then_some(Effect::Web(WebOp::Engine(tab.id, EngineOp::Zoom(next)))));
            fx.push(Effect::Emit(Event::TabUpdated(tab.id)));
        }
        fx
    }

    /// `site_js_set(host, on)`: remembers the choice and rebuilds the tabs of that host.
    pub fn site_js_set(&mut self, host: &str, on: bool) -> Vec<Effect> {
        self.prefs.set_js(host, on, self.settings.js_default);
        let mut fx = self.save_prefs();
        fx.extend(self.apply_js());
        fx
    }

    /// Rebuilds every live tab webview whose JS flag no longer matches its site.
    pub(super) fn apply_js(&mut self) -> Vec<Effect> {
        let stale: Vec<(u32, String)> = self
            .tabs
            .iter()
            .filter(|t| {
                t.web_js
                    .is_some_and(|js| host_of(&t.url).is_some_and(|h| self.js_of(&h) != js))
            })
            .map(|t| (t.id, t.url.clone()))
            .collect();
        let mut fx = Vec::new();
        for (id, url) in stale {
            fx.extend(self.load(id, &url));
            fx.push(Effect::Emit(Event::TabUpdated(id)));
        }
        fx.push(Effect::Emit(Event::TabsChanged));
        fx
    }
}
