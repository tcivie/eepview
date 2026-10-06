// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The router figures: the latest round of the sampler and the bandwidth history, saved in
//! `bandwidth.json` (docs/wiki/router-console.md R48–R53).

use super::{Core, Paths};
use crate::net::stats::{History, RouterStats, SAVE_EVERY_MS, Sample};
use crate::store::bandwidth;

/// The latest figures, the history, the time of the last save attempt, and whether a
/// sample came since the last save.
#[derive(Debug, Default)]
pub(super) struct StatsState {
    history: History,
    latest: RouterStats,
    tried_at: u64,
    unsaved: bool,
}

impl StatsState {
    /// The history saved in `paths` (empty without paths), with no figures yet. The first
    /// timed save comes [`SAVE_EVERY_MS`] after `now`.
    pub(super) fn load(paths: Option<&Paths>, now: u64) -> Self {
        Self {
            history: paths.map_or_else(History::default, |p| bandwidth::load(&p.bandwidth, now)),
            latest: RouterStats::default(),
            tried_at: now,
            unsaved: false,
        }
    }
}

impl Core {
    /// Records one router stats sample.
    pub fn record_stats(&mut self, now: u64, stats: &RouterStats) {
        let before = self.stats.history.newest();
        self.stats.history.record(now, stats);
        self.stats.unsaved |= self.stats.history.newest() != before;
    }

    /// Records a stats sample, unless the newest one is less than 4 s old.
    pub fn record_stats_spaced(&mut self, now: u64, stats: &RouterStats) {
        let before = self.stats.history.newest();
        self.stats.history.record_spaced(now, stats);
        self.stats.unsaved |= self.stats.history.newest() != before;
    }

    /// The router bandwidth that the history holds, oldest first.
    #[must_use]
    pub fn stats_history(&self) -> Vec<Sample> {
        self.stats.history.samples()
    }

    /// Keeps the figures of the last sampler round. Their `history` is not kept.
    pub fn set_latest_stats(&mut self, stats: RouterStats) {
        self.stats.latest = RouterStats {
            history: Vec::new(),
            ..stats
        };
    }

    /// The `router_stats()` answer: the latest figures and the bandwidth of the last 10
    /// minutes before `now`.
    #[must_use]
    pub fn stats_answer(&self, now: u64) -> RouterStats {
        RouterStats {
            history: self.stats.history.recent(now),
            ..self.stats.latest.clone()
        }
    }

    /// Saves the history when a sample came since the last save and the last attempt is at
    /// least [`SAVE_EVERY_MS`] before `now`. A failed attempt counts. True when it wrote the
    /// file.
    pub fn save_stats_if_due(&mut self, now: u64) -> bool {
        let due = now.saturating_sub(self.stats.tried_at) >= SAVE_EVERY_MS;
        self.stats.unsaved && due && self.save_stats(now)
    }

    /// Saves the history now (the quit save). True when it wrote the file; false without
    /// paths or when the write fails (the next try is [`SAVE_EVERY_MS`] later).
    pub fn save_stats(&mut self, now: u64) -> bool {
        let Some(path) = self.paths.as_ref().map(|p| p.bandwidth.clone()) else {
            return false;
        };
        self.stats.tried_at = now;
        let saved = bandwidth::save(&path, &self.stats.history).is_ok();
        self.stats.unsaved &= !saved;
        saved
    }
}
