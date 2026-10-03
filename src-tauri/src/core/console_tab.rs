// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The router console tab (`docs/wiki/router-console.md`, R23–R30).
//!
//! The console tab shows the `console` webview, which the shell builds only from a verified
//! console. The core never makes a [`WebOp`](super::WebOp) for it: its engine calls go out
//! as [`ConsoleOp`], so a console URL never reaches a `tab-*` webview.

use super::{ConsoleOp, Core, Effect, Event};
use crate::session::{Step, Traverse};
use crate::tabs::{Place, Tab};
use crate::types::{NavFlags, TabInfo, TabMarks};

/// The title of the console tab until its page sends one.
pub const CONSOLE_TITLE: &str = "Router console";
/// The warning when the console view cannot be confined (R19).
pub const CONSOLE_FAILED: &str = "The router console could not be opened safely";

impl Core {
    /// The id of the console tab, if one is open.
    #[must_use]
    pub fn console_tab(&self) -> Option<u32> {
        self.console_tab
    }

    /// R24: selects the console tab, or makes one right after the active tab and selects
    /// it, showing `url`. The shell has already started the load.
    pub fn console_open(&mut self, url: &str) -> Vec<Effect> {
        let known = self.console_tab();
        let mut fx = if known == Some(self.tabs.active_id()) {
            Vec::new()
        } else {
            self.leave_tab()
        };
        let id = known.unwrap_or_else(|| self.tabs.open(url, Place::AfterActive, true));
        self.console_tab = Some(id);
        self.tabs.select(id);
        if let Some(tab) = self.tabs.get_mut(id) {
            show_console_page(tab, url, known.is_none());
        }
        fx.extend([
            Effect::Emit(Event::TabsChanged),
            Effect::Layout,
            Effect::FocusContent,
        ]);
        fx
    }

    /// R25: the `console` webview started a main-frame load of `url`.
    pub fn console_started(&mut self, url: &str) -> Vec<Effect> {
        self.console_commit(url, true)
    }

    /// R25: the `console` webview finished a main-frame load of `url`.
    pub fn console_finished(&mut self, url: &str) -> Vec<Effect> {
        self.console_commit(url, false)
    }

    fn console_commit(&mut self, url: &str, loading: bool) -> Vec<Effect> {
        let Some(tab) = self.console_mut() else {
            return Vec::new();
        };
        url.clone_into(&mut tab.url);
        tab.loading = loading;
        tab.session.commit_web(url);
        vec![Effect::Emit(Event::TabUpdated(tab.id))]
    }

    /// R25: the document title of the console page changed.
    pub fn console_title(&mut self, title: &str) -> Vec<Effect> {
        let Some(tab) = self.console_mut().filter(|_| !title.trim().is_empty()) else {
            return Vec::new();
        };
        title.clone_into(&mut tab.title);
        vec![Effect::Emit(Event::TabUpdated(tab.id))]
    }

    /// R30: the console went away: closes the console tab, if open.
    pub fn console_gone(&mut self) -> Vec<Effect> {
        self.console_tab()
            .map(|id| self.tab_close(id))
            .unwrap_or_default()
    }

    /// R19: the console rule list could not be attached: the console tab closes, with a
    /// warning.
    pub fn console_failed(&mut self) -> Vec<Effect> {
        let mut fx = self.console_gone();
        fx.push(Self::toast("warn", CONSOLE_FAILED));
        fx
    }

    /// True while the console tab is the active tab.
    pub(super) fn console_active(&self) -> bool {
        self.console_tab == Some(self.tabs.active_id())
    }

    /// R26: before an address-bar navigation, a console tab becomes a fresh normal tab
    /// showing `landing`, and its `console` webview goes.
    pub(super) fn leave_console(&mut self, id: u32, landing: &str) -> Vec<Effect> {
        if self.console_tab != Some(id) || !self.tabs.reset(id, landing) {
            return Vec::new();
        }
        self.console_tab = None;
        vec![Effect::Console(ConsoleOp::Close)]
    }

    /// R27: back or forward in the console tab, through the `console` webview.
    pub(super) fn console_step(&mut self, id: u32, step: Step) -> Vec<Effect> {
        let Some(tab) = self.tabs.get_mut(id) else {
            return Vec::new();
        };
        let live = tab.url.clone();
        if !matches!(
            tab.session.step(step, Some(&live)),
            Some(Traverse::Engine(_))
        ) {
            return Vec::new();
        }
        let op = match step {
            Step::Back => ConsoleOp::Back,
            Step::Forward => ConsoleOp::Forward,
        };
        Self::console_engine(tab, op)
    }

    /// R27: reload of the console tab.
    pub(super) fn console_reload(&mut self, id: u32, hard: bool) -> Vec<Effect> {
        let op = if hard {
            ConsoleOp::HardReload
        } else {
            ConsoleOp::Reload
        };
        self.tabs
            .get_mut(id)
            .map(|tab| Self::console_engine(tab, op))
            .unwrap_or_default()
    }

    /// R27: stop of the console tab, while it loads.
    pub(super) fn console_stop(&mut self, id: u32) -> Vec<Effect> {
        let Some(tab) = self.tabs.get_mut(id).filter(|t| t.loading) else {
            return Vec::new();
        };
        tab.loading = false;
        vec![
            Effect::Console(ConsoleOp::Stop),
            Effect::Emit(Event::TabUpdated(id)),
        ]
    }

    fn console_engine(tab: &mut Tab, op: ConsoleOp) -> Vec<Effect> {
        tab.loading = true;
        vec![Effect::Console(op), Effect::Emit(Event::TabUpdated(tab.id))]
    }

    fn console_mut(&mut self) -> Option<&mut Tab> {
        let id = self.console_tab?;
        self.tabs.get_mut(id)
    }

    /// R23, R25: the `TabInfo` of the console tab.
    pub(super) fn console_info(&self, tab: &Tab) -> TabInfo {
        TabInfo {
            id: tab.id,
            url: tab.url.clone(),
            title: tab.title.clone(),
            kind: "console",
            nav: NavFlags {
                loading: tab.loading,
                can_back: tab.session.can_back(),
                can_forward: tab.session.can_forward(),
            },
            zoom: 1.0,
            marks: TabMarks {
                active: tab.id == self.tabs.active_id(),
                js_on: true,
                bookmarked: false,
            },
            icon: None,
        }
    }
}

/// Points `tab` at the console page `url`. A new console tab is titled [`CONSOLE_TITLE`]
/// until its page sends a title.
fn show_console_page(tab: &mut Tab, url: &str, fresh: bool) {
    if fresh {
        CONSOLE_TITLE.clone_into(&mut tab.title);
    }
    url.clone_into(&mut tab.url);
    tab.loading = true;
}

#[cfg(test)]
mod tests;
