// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The back/forward list of one tab. Pure logic.
//!
//! The list holds both `eepview://` and `http(s)://` entries. The engine of a `tab-*` webview
//! keeps its own list too, but it knows nothing of the internal pages and loses everything
//! when the webview is rebuilt. So this list is the truth, and `engine_back` / `engine_forward`
//! count only the steps where the engine list still matches it. Outside those steps the shell
//! loads the entry directly.

use crate::nav::is_web;

/// Most entries one tab remembers.
const MAX_ENTRIES: usize = 100;

/// A move through the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// One entry back.
    Back,
    /// One entry forward.
    Forward,
}

/// How the shell must carry out a [`Step`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Traverse {
    /// Ask the engine (`history.back()` or the native call); its list matches ours.
    Engine(Step),
    /// Load this entry directly; the load commits it.
    Load(String),
    /// Show this internal page; already committed.
    Internal(String),
    /// Show the live webview again; it still holds this entry. Already committed.
    Reveal(String),
}

/// The back/forward list of one tab.
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    entries: Vec<String>,
    index: usize,
    pending: Option<Step>,
    engine_back: usize,
    engine_forward: usize,
}

impl Session {
    /// A list with one entry.
    #[must_use]
    pub fn new(url: &str) -> Self {
        Self {
            entries: vec![url.to_owned()],
            index: 0,
            pending: None,
            engine_back: 0,
            engine_forward: 0,
        }
    }

    /// The current entry.
    #[must_use]
    pub fn current(&self) -> &str {
        &self.entries[self.index]
    }

    /// True when a back step exists.
    #[must_use]
    pub fn can_back(&self) -> bool {
        self.index > 0
    }

    /// True when a forward step exists.
    #[must_use]
    pub fn can_forward(&self) -> bool {
        self.index + 1 < self.entries.len()
    }

    fn neighbour(&self, step: Step) -> Option<usize> {
        match step {
            Step::Back => self.index.checked_sub(1),
            Step::Forward => (self.index + 1 < self.entries.len()).then_some(self.index + 1),
        }
    }

    /// Plans a step. `web_live` is the entry the tab webview shows now, when one exists.
    pub fn step(&mut self, step: Step, web_live: Option<&str>) -> Option<Traverse> {
        let target = self.neighbour(step)?;
        let url = self.entries[target].clone();
        if !is_web(&url) {
            self.index = target;
            return Some(Traverse::Internal(url));
        }
        if !is_web(self.current()) && web_live == Some(url.as_str()) {
            self.index = target;
            return Some(Traverse::Reveal(url));
        }
        self.pending = Some(step);
        let engine_steps = match step {
            Step::Back => self.engine_back,
            Step::Forward => self.engine_forward,
        };
        if is_web(self.current()) && engine_steps > 0 {
            Some(Traverse::Engine(step))
        } else {
            Some(Traverse::Load(url))
        }
    }

    /// Records a new internal page; it never reaches the engine.
    pub fn push_internal(&mut self, url: &str) {
        self.pending = None;
        if self.current() != url {
            self.push(url);
        }
    }

    /// Puts `url` in place of the current entry: an error page takes the place of the load
    /// that failed, and a new try takes the place of the error page. The engine list no
    /// longer matches this list, so later steps load their entry.
    pub fn replace_current(&mut self, url: &str) {
        self.pending = None;
        url.clone_into(&mut self.entries[self.index]);
        self.engine_reset();
    }

    /// The tab webview was built again: its engine list is empty.
    pub fn engine_reset(&mut self) {
        self.engine_back = 0;
        self.engine_forward = 0;
    }

    /// A web page committed in the tab webview at `url`.
    pub fn commit_web(&mut self, url: &str) {
        match self.pending.take() {
            Some(step) => self.commit_step(step, url),
            None => self.commit_new(url),
        }
    }

    fn commit_step(&mut self, step: Step, url: &str) {
        let expected = self.neighbour(step);
        if expected.is_some_and(|i| self.entries[i] == url) {
            self.apply_engine_step(step);
        } else {
            self.commit_new(url);
        }
    }

    fn apply_engine_step(&mut self, step: Step) {
        let was_web = is_web(self.current());
        let engine_steps = match step {
            Step::Back => self.engine_back,
            Step::Forward => self.engine_forward,
        };
        if was_web && engine_steps > 0 {
            self.move_engine(step);
        } else {
            self.engine_reset();
        }
        match step {
            Step::Back => self.index -= 1,
            Step::Forward => self.index += 1,
        }
    }

    fn move_engine(&mut self, step: Step) {
        match step {
            Step::Back => {
                self.engine_back -= 1;
                self.engine_forward += 1;
            }
            Step::Forward => {
                self.engine_forward -= 1;
                self.engine_back += 1;
            }
        }
    }

    fn commit_new(&mut self, url: &str) {
        if self.current() == url {
            return;
        }
        if is_web(self.current()) {
            self.engine_back += 1;
        } else {
            self.engine_back = 0;
        }
        self.engine_forward = 0;
        self.push(url);
    }

    fn push(&mut self, url: &str) {
        self.entries.truncate(self.index + 1);
        self.entries.push(url.to_owned());
        if self.entries.len() > MAX_ENTRIES {
            self.entries.remove(0);
            self.engine_back = self.engine_back.min(MAX_ENTRIES - 1);
        }
        self.index = self.entries.len() - 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "http://a.i2p/";
    const B: &str = "http://b.i2p/";
    const C: &str = "http://c.i2p/";
    const HOME: &str = "eepview://home";

    fn web_session(urls: &[&str]) -> Session {
        let mut s = Session::new(urls[0]);
        for url in &urls[1..] {
            s.commit_web(url);
        }
        s
    }

    #[test]
    fn new_entries_and_reload() {
        let mut s = web_session(&[A, B]);
        assert_eq!(s.current(), B);
        assert!(s.can_back() && !s.can_forward());
        s.commit_web(B);
        assert_eq!(s.entries.len(), 2);
    }

    #[test]
    fn engine_back_and_forward_between_web_pages() {
        let mut s = web_session(&[A, B, C]);
        assert_eq!(
            s.step(Step::Back, Some(C)),
            Some(Traverse::Engine(Step::Back))
        );
        s.commit_web(B);
        assert_eq!(s.current(), B);
        assert!(s.can_forward());
        assert_eq!(
            s.step(Step::Forward, Some(B)),
            Some(Traverse::Engine(Step::Forward))
        );
        s.commit_web(C);
        assert_eq!(s.current(), C);
    }

    #[test]
    fn new_page_after_back_drops_forward() {
        let mut s = web_session(&[A, B]);
        s.step(Step::Back, Some(B));
        s.commit_web(A);
        s.commit_web(C);
        assert_eq!(s.entries, vec![A, C]);
        assert!(!s.can_forward());
    }

    #[test]
    fn internal_entries_are_shown_directly() {
        let mut s = Session::new(HOME);
        s.commit_web(A);
        assert_eq!(
            s.step(Step::Back, Some(A)),
            Some(Traverse::Internal(HOME.into()))
        );
        assert_eq!(s.current(), HOME);
        assert_eq!(
            s.step(Step::Forward, Some(A)),
            Some(Traverse::Reveal(A.into()))
        );
        assert_eq!(s.current(), A);
    }

    #[test]
    fn web_entry_without_live_webview_loads() {
        let mut s = Session::new(A);
        s.push_internal(HOME);
        assert_eq!(s.step(Step::Back, None), Some(Traverse::Load(A.into())));
        s.commit_web(A);
        assert_eq!(s.current(), A);
        assert_eq!(s.index, 0);
    }

    #[test]
    fn rebuilt_webview_loads_instead_of_engine() {
        let mut s = web_session(&[A, B]);
        s.engine_reset();
        assert_eq!(s.step(Step::Back, Some(B)), Some(Traverse::Load(A.into())));
        s.commit_web(A);
        assert_eq!(s.current(), A);
        assert_eq!(s.engine_back, 0);
    }

    #[test]
    fn web_after_internal_does_not_trust_engine() {
        let mut s = Session::new(A);
        s.push_internal(HOME);
        s.commit_web(B);
        assert_eq!(
            s.step(Step::Back, Some(B)),
            Some(Traverse::Internal(HOME.into()))
        );
        s.commit_web(C);
        assert_eq!(
            s.step(Step::Back, Some(C)),
            Some(Traverse::Internal(HOME.into()))
        );
    }

    #[test]
    fn unexpected_url_after_step_is_a_new_entry() {
        let mut s = web_session(&[A, B]);
        s.step(Step::Back, Some(B));
        s.commit_web(C);
        assert_eq!(s.entries, vec![A, B, C]);
    }

    #[test]
    fn no_step_at_the_ends() {
        let mut s = Session::new(A);
        assert_eq!(s.step(Step::Back, Some(A)), None);
        assert_eq!(s.step(Step::Forward, Some(A)), None);
        s.push_internal(A);
        assert_eq!(s.entries.len(), 1);
    }

    #[test]
    fn list_is_capped() {
        let mut s = Session::new(A);
        for i in 0..150 {
            s.commit_web(&format!("http://p{i}.i2p/"));
        }
        assert_eq!(s.entries.len(), MAX_ENTRIES);
        assert_eq!(s.current(), "http://p149.i2p/");
        assert!(s.engine_back < MAX_ENTRIES);
    }
}
