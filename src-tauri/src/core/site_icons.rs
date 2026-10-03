// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! When to ask a site for its icon, and when to forget it (`docs/wiki/site-icons.md`).

use std::collections::{BTreeSet, HashMap, VecDeque};

use super::{Core, Effect, Event};
use crate::icons::{Icon, MAX_IN_FLIGHT, REFETCH_MS};
use crate::nav::host_of;
use crate::types::{Bookmark, HistoryEntry};

/// The fetches that run and the hosts that wait for a free slot.
#[derive(Debug, Default)]
pub(super) struct IconJobs {
    running: BTreeSet<String>,
    waiting: VecDeque<String>,
}

impl Core {
    /// Asks for the icon of the host of `url` when it is due: a bookmarked or visited host,
    /// not asked in the last 24 h, not running and not waiting already.
    pub(super) fn want_icon(&mut self, url: &str, now: u64) -> Vec<Effect> {
        let Some(host) = host_of(url) else {
            return Vec::new();
        };
        if !self.icon_due(&host, now) || self.icon_jobs.waiting.contains(&host) {
            return Vec::new();
        }
        if self.icon_jobs.running.len() >= MAX_IN_FLIGHT {
            self.icon_jobs.waiting.push_back(host);
            return Vec::new();
        }
        self.start_icon(host, now)
    }

    fn icon_due(&self, host: &str, now: u64) -> bool {
        let asked = self
            .icons
            .last_attempt(host)
            .is_some_and(|t| now.saturating_sub(t) < REFETCH_MS);
        !asked && !self.icon_jobs.running.contains(host) && self.host_is_live(host)
    }

    fn start_icon(&mut self, host: String, now: u64) -> Vec<Effect> {
        let mut fx = Self::saved(self.icons.record_attempt(&host, now), "site icons");
        self.icon_jobs.running.insert(host.clone());
        fx.push(Effect::FetchIcon(host));
        fx
    }

    /// A fetch for `host` ended: `Some` with the sanitized icon, `None` on any failure.
    /// Stores the icon, tells the UI, and starts the next waiting host.
    pub fn icon_fetched(&mut self, host: &str, icon: Option<Icon>, now: u64) -> Vec<Effect> {
        self.icon_jobs.running.remove(host);
        let mut fx = Vec::new();
        if let Some(icon) = icon.filter(|_| self.host_is_live(host)) {
            fx.extend(Self::saved(self.icons.put(host, &icon), "site icons"));
            fx.extend(icons_changed());
        }
        fx.extend(self.next_icon(now));
        fx
    }

    /// Starts the first waiting host that is still due; drops the ones before it.
    fn next_icon(&mut self, now: u64) -> Vec<Effect> {
        if self.icon_jobs.running.len() >= MAX_IN_FLIGHT {
            return Vec::new();
        }
        let mut waiting = std::mem::take(&mut self.icon_jobs.waiting).into_iter();
        let next = waiting.by_ref().find(|h| self.icon_due(h, now));
        self.icon_jobs.waiting = waiting.collect();
        next.map_or_else(Vec::new, |host| self.start_icon(host, now))
    }

    /// True while `host` has a bookmark or a history entry.
    fn host_is_live(&self, host: &str) -> bool {
        let same = |url: &str| host_of(url).is_some_and(|h| h == host);
        self.bookmarks.list().iter().any(|b| same(&b.url))
            || self.history.entries().iter().any(|e| same(&e.url))
    }

    /// Every host with a bookmark or a history entry.
    fn live_hosts(&self) -> Vec<String> {
        let bookmarks = self.bookmarks.list().iter().map(|b| b.url.as_str());
        let history = self.history.entries().iter().map(|e| e.url.as_str());
        let hosts: BTreeSet<String> = bookmarks.chain(history).filter_map(host_of).collect();
        hosts.into_iter().collect()
    }

    /// Deletes the icons of hosts with no bookmark and no history entry left.
    pub(super) fn sweep_icons(&mut self) -> Vec<Effect> {
        let live = self.live_hosts();
        match self.icons.retain(&live) {
            Ok(true) => icons_changed(),
            Ok(false) => Vec::new(),
            Err(e) => vec![Self::toast(
                "warn",
                format!("Could not clear site icons: {e}"),
            )],
        }
    }

    /// The 32 px icon of a page's host.
    pub(super) fn small_icon(&self, url: &str) -> Option<String> {
        host_of(url).and_then(|h| self.icons.small(&h))
    }

    /// A bookmark with its 64 px icon, for the UI.
    pub(super) fn with_icon(&self, mut bookmark: Bookmark) -> Bookmark {
        bookmark.icon = host_of(&bookmark.url).and_then(|h| self.icons.large(&h));
        bookmark
    }

    /// History entries with their 32 px icons, each host read once.
    pub(super) fn with_icons(&self, entries: Vec<HistoryEntry>) -> Vec<HistoryEntry> {
        let mut seen: HashMap<String, Option<String>> = HashMap::new();
        entries
            .into_iter()
            .map(|mut entry| {
                let host = host_of(&entry.url).unwrap_or_default();
                let icon = seen
                    .entry(host)
                    .or_insert_with_key(|h| self.icons.small(h))
                    .clone();
                entry.icon = icon;
                entry
            })
            .collect()
    }
}

/// `icons-changed`, then `tabs-changed` for the tab icons.
fn icons_changed() -> Vec<Effect> {
    vec![Effect::Emit(Event::Icons), Effect::Emit(Event::TabsChanged)]
}
