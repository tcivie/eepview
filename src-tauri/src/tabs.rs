// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The tab strip model. Pure logic.

use crate::session::Session;

/// Most closed tabs that "reopen closed tab" remembers.
const MAX_CLOSED: usize = 25;

/// One tab.
#[derive(Debug, Clone, PartialEq)]
pub struct Tab {
    /// Stable id, never reused in one run.
    pub id: u32,
    /// What the address bar shows: `eepview://…` or `http(s)://…`.
    pub url: String,
    /// Page title.
    pub title: String,
    /// A page load is in progress.
    pub loading: bool,
    /// Back/forward list.
    pub session: Session,
    /// The JS flag of the live `tab-*` webview, when one exists.
    pub web_js: Option<bool>,
    /// The URL the live `tab-*` webview shows.
    pub web_url: Option<String>,
    /// The title of the page in the live `tab-*` webview.
    pub web_title: String,
}

impl Tab {
    fn new(id: u32, url: &str) -> Self {
        Self {
            id,
            url: url.to_owned(),
            title: String::new(),
            loading: false,
            session: Session::new(url),
            web_js: None,
            web_url: None,
            web_title: String::new(),
        }
    }
}

/// Where a new tab goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// At the end of the strip.
    End,
    /// Right after the active tab (links that open a new tab).
    AfterActive,
}

#[derive(Debug, Clone, PartialEq)]
struct Closed {
    url: String,
    index: usize,
}

/// The tab strip.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tabs {
    list: Vec<Tab>,
    active: u32,
    next_id: u32,
    closed: Vec<Closed>,
}

impl Tabs {
    /// An empty strip.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens a tab and returns its id.
    pub fn open(&mut self, url: &str, place: Place, activate: bool) -> u32 {
        let index = match place {
            Place::End => self.list.len(),
            Place::AfterActive => self
                .index_of(self.active)
                .map_or(self.list.len(), |i| i + 1),
        };
        self.insert(url, index, activate)
    }

    fn insert(&mut self, url: &str, index: usize, activate: bool) -> u32 {
        self.next_id += 1;
        let id = self.next_id;
        self.list
            .insert(index.min(self.list.len()), Tab::new(id, url));
        if activate || self.list.len() == 1 {
            self.active = id;
        }
        id
    }

    /// Closes a tab. Closing the last tab opens `home`. Returns the closed tab.
    pub fn close(&mut self, id: u32, home: &str) -> Option<Tab> {
        let index = self.index_of(id)?;
        let tab = self.list.remove(index);
        self.closed.push(Closed {
            url: tab.url.clone(),
            index,
        });
        if self.closed.len() > MAX_CLOSED {
            self.closed.remove(0);
        }
        if self.list.is_empty() {
            self.insert(home, 0, true);
        } else if self.active == id {
            self.active = self.list[index.min(self.list.len() - 1)].id;
        }
        Some(tab)
    }

    /// Makes a tab active. False when it does not exist.
    pub fn select(&mut self, id: u32) -> bool {
        let found = self.index_of(id).is_some();
        if found {
            self.active = id;
        }
        found
    }

    /// Moves a tab to `index` (clamped). False when it does not exist.
    pub fn move_to(&mut self, id: u32, index: usize) -> bool {
        let Some(from) = self.index_of(id) else {
            return false;
        };
        let tab = self.list.remove(from);
        self.list.insert(index.min(self.list.len()), tab);
        true
    }

    /// Opens the most recently closed tab again, at its old place.
    pub fn reopen(&mut self) -> Option<u32> {
        let closed = self.closed.pop()?;
        Some(self.insert(&closed.url, closed.index, true))
    }

    /// Selects the next (`forward`) or previous tab, wrapping around.
    pub fn cycle(&mut self, forward: bool) -> Option<u32> {
        let len = self.list.len();
        let index = self.index_of(self.active)?;
        let next = if forward {
            (index + 1) % len
        } else {
            (index + len - 1) % len
        };
        self.active = self.list[next].id;
        Some(self.active)
    }

    /// Selects tab `n` (1-based) for n = 1..8; n = 9 selects the last tab.
    pub fn select_number(&mut self, n: usize) -> Option<u32> {
        let index = if n == 9 {
            self.list.len().checked_sub(1)?
        } else {
            n.checked_sub(1)?
        };
        let id = self.list.get(index)?.id;
        self.active = id;
        Some(id)
    }

    fn index_of(&self, id: u32) -> Option<usize> {
        self.list.iter().position(|t| t.id == id)
    }

    /// The active tab id (0 when the strip is empty).
    #[must_use]
    pub fn active_id(&self) -> u32 {
        self.active
    }

    /// A tab by id.
    #[must_use]
    pub fn get(&self, id: u32) -> Option<&Tab> {
        self.list.iter().find(|t| t.id == id)
    }

    /// A tab by id, mutable.
    pub fn get_mut(&mut self, id: u32) -> Option<&mut Tab> {
        self.list.iter_mut().find(|t| t.id == id)
    }

    /// The active tab.
    #[must_use]
    pub fn active(&self) -> Option<&Tab> {
        self.get(self.active)
    }

    /// All tabs in strip order.
    pub fn iter(&self) -> impl Iterator<Item = &Tab> {
        self.list.iter()
    }

    /// All tabs in strip order, mutable.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Tab> {
        self.list.iter_mut()
    }

    /// Number of tabs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.list.len()
    }

    /// True when no tab is open.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "eepview://home";

    fn ids(t: &Tabs) -> Vec<u32> {
        t.iter().map(|t| t.id).collect()
    }

    fn three() -> Tabs {
        let mut t = Tabs::new();
        t.open("http://a.i2p/", Place::End, true);
        t.open("http://b.i2p/", Place::End, false);
        t.open("http://c.i2p/", Place::End, false);
        t
    }

    #[test]
    fn open_places_and_activation() {
        let mut t = three();
        assert_eq!(ids(&t), vec![1, 2, 3]);
        assert_eq!(t.active_id(), 1);
        let id = t.open("http://d.i2p/", Place::AfterActive, false);
        assert_eq!(ids(&t), vec![1, 4, 2, 3]);
        assert_eq!(t.active_id(), 1);
        assert_eq!(t.get(id).map(|x| x.url.as_str()), Some("http://d.i2p/"));
        assert_eq!(t.len(), 4);
        assert!(!t.is_empty());
    }

    #[test]
    fn first_tab_is_active_even_in_background() {
        let mut t = Tabs::new();
        assert!(t.is_empty());
        t.open(HOME, Place::AfterActive, false);
        assert_eq!(t.active_id(), 1);
        assert_eq!(t.active().map(|x| x.url.as_str()), Some(HOME));
    }

    #[test]
    fn close_active_selects_right_then_left() {
        let mut t = three();
        t.select(2);
        t.close(2, HOME);
        assert_eq!(t.active_id(), 3);
        t.close(3, HOME);
        assert_eq!(t.active_id(), 1);
        assert!(t.close(99, HOME).is_none());
    }

    #[test]
    fn close_background_keeps_active() {
        let mut t = three();
        t.close(3, HOME);
        assert_eq!(t.active_id(), 1);
    }

    #[test]
    fn close_last_opens_home() {
        let mut t = Tabs::new();
        t.open("http://a.i2p/", Place::End, true);
        let closed = t.close(1, HOME).unwrap();
        assert_eq!(closed.url, "http://a.i2p/");
        assert_eq!(t.len(), 1);
        assert_eq!(t.active().unwrap().url, HOME);
        assert_eq!(t.active_id(), 2);
    }

    #[test]
    fn reopen_restores_place_and_url() {
        let mut t = three();
        t.close(2, HOME);
        let id = t.reopen().unwrap();
        assert_eq!(ids(&t), vec![1, id, 3]);
        assert_eq!(t.active_id(), id);
        assert_eq!(t.get(id).unwrap().url, "http://b.i2p/");
        assert!(t.reopen().is_none());
    }

    #[test]
    fn closed_list_is_capped() {
        let mut t = Tabs::new();
        for _ in 0..30 {
            let id = t.open("http://a.i2p/", Place::End, true);
            t.close(id, HOME);
        }
        assert_eq!(t.closed.len(), MAX_CLOSED);
    }

    #[test]
    fn select_and_move() {
        let mut t = three();
        assert!(t.select(3));
        assert!(!t.select(9));
        assert!(t.move_to(3, 0));
        assert_eq!(ids(&t), vec![3, 1, 2]);
        assert!(t.move_to(3, 99));
        assert_eq!(ids(&t), vec![1, 2, 3]);
        assert!(!t.move_to(9, 0));
    }

    #[test]
    fn cycle_wraps() {
        let mut t = three();
        assert_eq!(t.cycle(false), Some(3));
        assert_eq!(t.cycle(true), Some(1));
        assert_eq!(t.cycle(true), Some(2));
        assert_eq!(Tabs::new().cycle(true), None);
    }

    #[test]
    fn select_by_number() {
        let mut t = three();
        assert_eq!(t.select_number(2), Some(2));
        assert_eq!(t.select_number(9), Some(3));
        assert_eq!(t.select_number(5), None);
        assert_eq!(t.select_number(0), None);
        assert_eq!(Tabs::new().select_number(9), None);
    }

    #[test]
    fn mutable_access() {
        let mut t = three();
        t.get_mut(2).unwrap().title = "B".into();
        for tab in t.iter_mut() {
            tab.loading = true;
        }
        assert_eq!(t.get(2).unwrap().title, "B");
        assert!(t.iter().all(|x| x.loading));
    }
}
