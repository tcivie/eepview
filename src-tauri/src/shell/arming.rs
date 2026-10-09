// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The engine filter (L3b) of each tab webview, and the load that waits for it (ADR 0001).
//!
//! A tab webview starts on `about:blank`. It loads a page only after its engine filter is
//! attached: then it is "armed". Until then, a load only replaces the URL that waits, and
//! when the filter is on, the newest URL that waits loads. When the filter cannot be
//! attached, the webview never loads a page: the next load builds a new webview, which tries
//! the filter again.

use std::collections::HashMap;

use tauri::Url;

/// The filter state of one tab webview.
#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    /// The filter is not on yet. The URL waits for it.
    Waiting(Url),
    /// The filter is on.
    Armed,
    /// The filter could not be attached.
    Failed,
}

/// What the shell does with a load in a tab webview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    /// The filter is on: load the URL now.
    Navigate,
    /// The filter is not on yet: the URL waits for it.
    Wait,
    /// The filter failed, or the webview is not known: build a new webview.
    Rebuild,
}

/// The filter state of every tab webview, by webview label.
#[derive(Debug, Default)]
pub struct Arming {
    views: HashMap<String, State>,
}

impl Arming {
    /// A new webview `label` whose filter is not on yet. `url` waits for the filter.
    pub fn start(&mut self, label: &str, url: Url) {
        self.views.insert(label.to_owned(), State::Waiting(url));
    }

    /// A load of `url` in the webview `label`. While the filter is not on, `url` replaces the
    /// URL that waits.
    pub fn load(&mut self, label: &str, url: &Url) -> Next {
        match self.views.get_mut(label) {
            Some(State::Armed) => Next::Navigate,
            Some(State::Waiting(waiting)) => {
                url.clone_into(waiting);
                Next::Wait
            }
            Some(State::Failed) | None => Next::Rebuild,
        }
    }

    /// The filter of `label` is on. Returns the URL that waited, which loads now. A webview
    /// that is not known or does not wait gets nothing, and a failed one stays failed.
    pub fn armed(&mut self, label: &str) -> Option<Url> {
        let state = self.views.get_mut(label)?;
        if !matches!(state, State::Waiting(_)) {
            return None;
        }
        match std::mem::replace(state, State::Armed) {
            State::Waiting(url) => Some(url),
            State::Armed | State::Failed => None,
        }
    }

    /// The filter of `label` could not be attached. Returns the URL that waited: it never
    /// loads in this webview.
    pub fn failed(&mut self, label: &str) -> Option<Url> {
        let state = self.views.get_mut(label)?;
        match std::mem::replace(state, State::Failed) {
            State::Waiting(url) => Some(url),
            State::Armed | State::Failed => None,
        }
    }

    /// The webview `label` is gone.
    pub fn forget(&mut self, label: &str) {
        self.views.remove(label);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        Url::parse(text).unwrap()
    }

    #[test]
    fn a_load_before_the_filter_waits_and_replaces_the_url() {
        let mut arming = Arming::default();
        arming.start("tab-1-0", url("http://a.i2p/"));
        assert_eq!(arming.load("tab-1-0", &url("http://b.i2p/")), Next::Wait);
        assert_eq!(arming.load("tab-1-0", &url("http://c.i2p/")), Next::Wait);
        assert_eq!(arming.armed("tab-1-0"), Some(url("http://c.i2p/")));
    }

    #[test]
    fn an_armed_webview_navigates_at_once() {
        let mut arming = Arming::default();
        arming.start("tab-1-0", url("http://a.i2p/"));
        assert_eq!(arming.armed("tab-1-0"), Some(url("http://a.i2p/")));
        assert_eq!(
            arming.load("tab-1-0", &url("http://b.i2p/")),
            Next::Navigate
        );
        assert_eq!(arming.armed("tab-1-0"), None, "the filter arms once");
    }

    #[test]
    fn a_failed_filter_never_loads_and_rebuilds_on_the_next_load() {
        let mut arming = Arming::default();
        arming.start("tab-1-0", url("http://a.i2p/"));
        arming.load("tab-1-0", &url("http://b.i2p/"));
        assert_eq!(arming.failed("tab-1-0"), Some(url("http://b.i2p/")));
        assert_eq!(
            arming.armed("tab-1-0"),
            None,
            "a failed webview stays failed"
        );
        assert_eq!(arming.load("tab-1-0", &url("http://c.i2p/")), Next::Rebuild);
        assert_eq!(arming.failed("tab-1-0"), None);
    }

    #[test]
    fn an_unknown_or_forgotten_webview_is_rebuilt() {
        let mut arming = Arming::default();
        assert_eq!(arming.load("tab-1-0", &url("http://a.i2p/")), Next::Rebuild);
        assert_eq!(arming.armed("tab-1-0"), None);
        assert_eq!(arming.failed("tab-1-0"), None);
        arming.start("tab-1-0", url("http://a.i2p/"));
        arming.forget("tab-1-0");
        assert_eq!(
            arming.armed("tab-1-0"),
            None,
            "a closed webview never loads"
        );
        assert_eq!(arming.load("tab-1-0", &url("http://b.i2p/")), Next::Rebuild);
    }

    #[test]
    fn each_webview_has_its_own_state() {
        let mut arming = Arming::default();
        arming.start("tab-1-0", url("http://a.i2p/"));
        arming.start("tab-2-1", url("http://b.i2p/"));
        arming.failed("tab-1-0");
        assert_eq!(arming.armed("tab-2-1"), Some(url("http://b.i2p/")));
        assert_eq!(arming.load("tab-1-0", &url("http://c.i2p/")), Next::Rebuild);
        assert_eq!(
            arming.load("tab-2-1", &url("http://c.i2p/")),
            Next::Navigate
        );
    }
}
