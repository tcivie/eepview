// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Tab commands: new, close, select, move, reopen, cycle.

use super::{Core, Effect, EngineOp, Event, WebOp};
use crate::nav::classify;
use crate::tabs::Place;
use crate::types::TabInfo;

impl Core {
    /// `tab_new(url?)`: opens a tab (home by default), makes it active, and returns it.
    pub fn tab_new(&mut self, input: Option<&str>, place: Place) -> (Option<TabInfo>, Vec<Effect>) {
        let raw = input.map_or_else(|| "eepview://home".to_owned(), str::to_owned);
        let target = classify(&raw);
        let landing = Self::landing(&target, &raw);
        let mut fx = self.leave_tab();
        let id = self.tabs.open(&landing, place, true);
        fx.extend(self.open_target(id, &target, &raw));
        fx.extend([Effect::Emit(Event::TabsChanged), Effect::Layout]);
        if input.is_none() {
            // A blank new tab (Cmd/Ctrl+T, the + button): type an address at once.
            self.focus_on_commit = None;
            fx.extend([
                Effect::Emit(Event::Shortcut("focus-address")),
                Effect::FocusToolbar,
            ]);
        }
        (self.tab_info(id), fx)
    }

    /// `tab_close(id)`: closing the last tab opens a home tab.
    pub fn tab_close(&mut self, id: u32) -> Vec<Effect> {
        let was_active = self.tabs.active_id() == id;
        let mut fx = if was_active {
            self.leave_tab()
        } else {
            Vec::new()
        };
        let home = self.home_url();
        let Some(closed) = self.tabs.close(id, &home) else {
            return fx;
        };
        if closed.web_js.is_some() {
            fx.push(Effect::Web(WebOp::Destroy(id)));
        }
        fx.extend(self.arrive());
        fx
    }

    /// `tab_select(id)`.
    pub fn tab_select(&mut self, id: u32) -> Vec<Effect> {
        if id == self.tabs.active_id() || self.tabs.get(id).is_none() {
            return Vec::new();
        }
        let mut fx = self.leave_tab();
        self.tabs.select(id);
        fx.extend(self.arrive());
        fx
    }

    /// `tab_move(id, index)`.
    pub fn tab_move(&mut self, id: u32, index: usize) -> Vec<Effect> {
        if self.tabs.move_to(id, index) {
            vec![Effect::Emit(Event::TabsChanged)]
        } else {
            Vec::new()
        }
    }

    /// Reopens the last closed tab.
    pub fn tab_reopen(&mut self) -> Vec<Effect> {
        let mut fx = self.leave_tab();
        if self.tabs.reopen().is_none() {
            return fx;
        }
        let id = self.tabs.active_id();
        let url = self.tabs.get(id).map(|t| t.url.clone()).unwrap_or_default();
        fx.extend(self.open_target(id, &classify(&url), &url));
        fx.extend(self.arrive());
        fx
    }

    /// Next or previous tab.
    pub fn tab_cycle(&mut self, forward: bool) -> Vec<Effect> {
        let mut fx = self.leave_tab();
        self.tabs.cycle(forward);
        fx.extend(self.arrive());
        fx
    }

    /// Tab 1–8, or the last tab for 9.
    pub fn tab_number(&mut self, n: usize) -> Vec<Effect> {
        let mut fx = self.leave_tab();
        self.tabs.select_number(n);
        fx.extend(self.arrive());
        fx
    }

    /// Clears per-tab UI state (find, hover) before the active tab changes.
    pub(super) fn leave_tab(&mut self) -> Vec<Effect> {
        self.link_run = None;
        let mut fx = Vec::new();
        if let Some(find) = self.find.take() {
            fx.push(Effect::Web(WebOp::Engine(find.tab, EngineOp::FindClear)));
        }
        if self.hover.leave() {
            fx.push(Effect::Emit(Event::Hover(None)));
        }
        fx
    }

    /// Effects after the active tab changed: load it when lazy, tell the UI, lay out.
    fn arrive(&mut self) -> Vec<Effect> {
        let id = self.tabs.active_id();
        let mut fx = self.ensure_loaded(id);
        fx.extend([Effect::Emit(Event::TabsChanged), Effect::Layout]);
        fx
    }
}
