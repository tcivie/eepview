// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Browsing history: newest first, capped at 10 000 entries, one entry per URL.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{VERSION, new_id, read_json, write_json};
#[cfg(test)]
use crate::types::Cursor;
use crate::types::{HistoryEntry, HistoryQuery};

/// Most entries kept.
pub const MAX_ENTRIES: usize = 10_000;
/// Entries `history_query` returns when the caller gives no limit.
const DEFAULT_LIMIT: usize = 200;
const HOUR_MS: u64 = 3_600_000;

#[derive(Debug, Serialize, Deserialize)]
struct File {
    version: u32,
    entries: Vec<HistoryEntry>,
}

/// The visit list, newest first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct History {
    entries: Vec<HistoryEntry>,
    seq: u64,
}

/// The age limit of a `history_clear` range in ms; `None` for "all". `Err` for an unknown name.
///
/// # Errors
///
/// Fails for a name other than `hour`, `day`, `week` or `all`.
pub fn range_ms(range: &str) -> Result<Option<u64>, String> {
    match range {
        "hour" => Ok(Some(HOUR_MS)),
        "day" => Ok(Some(24 * HOUR_MS)),
        "week" => Ok(Some(7 * 24 * HOUR_MS)),
        "all" => Ok(None),
        other => Err(format!("unknown range: {other}")),
    }
}

impl History {
    /// Reads the file, or starts empty.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        read_json::<File>(path).map_or_else(Self::default, |f| Self::checked(f.entries))
    }

    /// A hand-edited file is not trusted: only I2P addresses, newest first, at most
    /// [`MAX_ENTRIES`].
    fn checked(mut entries: Vec<HistoryEntry>) -> Self {
        entries.retain(|e| crate::nav::is_allowed(&e.url));
        entries.sort_by_key(|e| std::cmp::Reverse(e.visited));
        entries.truncate(MAX_ENTRIES);
        Self {
            seq: entries.len() as u64,
            entries,
        }
    }

    /// Writes the file.
    ///
    /// # Errors
    ///
    /// Fails when the file cannot be written.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        write_json(
            path,
            &File {
                version: VERSION,
                entries: self.entries.clone(),
            },
        )
    }

    /// All entries, newest first.
    #[must_use]
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Records a visit: moves the URL to the top and counts it.
    pub fn visit(&mut self, url: &str, title: &str, now: u64) {
        let entry = match self.entries.iter().position(|e| e.url == url) {
            Some(i) => revisit(self.entries.remove(i), title, now),
            None => self.fresh(url, title, now),
        };
        self.entries.insert(0, entry);
        self.entries.truncate(MAX_ENTRIES);
    }

    fn fresh(&mut self, url: &str, title: &str, now: u64) -> HistoryEntry {
        self.seq += 1;
        HistoryEntry {
            id: new_id(now, self.seq),
            url: url.to_owned(),
            title: title.to_owned(),
            visited: now,
            visits: 1,
        }
    }

    /// Sets the title of a URL that is already in the list. False when it is not.
    pub fn set_title(&mut self, url: &str, title: &str) -> bool {
        match self.entries.iter_mut().find(|e| e.url == url) {
            Some(e) if e.title != title => {
                title.clone_into(&mut e.title);
                true
            }
            _ => false,
        }
    }

    /// Entries that match the query, newest first.
    #[must_use]
    pub fn query(&self, q: &HistoryQuery) -> Vec<HistoryEntry> {
        let needle = q.q.as_deref().unwrap_or("").trim().to_lowercase();
        let mut hits: Vec<&HistoryEntry> = self
            .entries
            .iter()
            .filter(|e| q.before.as_ref().is_none_or(|c| c.admits(e.visited, &e.id)))
            .filter(|e| needle.is_empty() || matches_text(e, &needle))
            .collect();
        hits.sort_by(|a, b| (b.visited, &b.id).cmp(&(a.visited, &a.id)));
        hits.into_iter()
            .take(q.limit.unwrap_or(DEFAULT_LIMIT))
            .cloned()
            .collect()
    }

    /// Removes one entry. False when there is none.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() != before
    }

    /// Removes the entries of the last `age` ms, or all of them for `None`.
    pub fn clear(&mut self, age: Option<u64>, now: u64) {
        match age {
            Some(ms) => {
                let since = now.saturating_sub(ms);
                self.entries.retain(|e| e.visited < since);
            }
            None => self.entries.clear(),
        }
    }
}

fn revisit(mut e: HistoryEntry, title: &str, now: u64) -> HistoryEntry {
    e.visits += 1;
    e.visited = now;
    if !title.is_empty() {
        title.clone_into(&mut e.title);
    }
    e
}

fn matches_text(e: &HistoryEntry, needle: &str) -> bool {
    e.url.to_lowercase().contains(needle) || e.title.to_lowercase().contains(needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::testdir;

    fn q(text: Option<&str>, before: Option<u64>, limit: Option<usize>) -> HistoryQuery {
        HistoryQuery {
            q: text.map(str::to_owned),
            before: before.map(Cursor::Time),
            limit,
        }
    }

    #[test]
    fn visits_move_to_top_and_count() {
        let mut h = History::default();
        h.visit("http://a.i2p/", "A", 1);
        h.visit("http://b.i2p/", "B", 2);
        h.visit("http://a.i2p/", "", 3);
        let e = h.entries();
        assert_eq!(e[0].url, "http://a.i2p/");
        assert_eq!(e[0].visits, 2);
        assert_eq!(e[0].visited, 3);
        assert_eq!(e[0].title, "A");
        assert_eq!(e.len(), 2);
    }

    #[test]
    fn capped_at_max() {
        let mut h = History::default();
        for i in 0..(MAX_ENTRIES as u64 + 5) {
            h.visit(&format!("http://p{i}.i2p/"), "", i);
        }
        assert_eq!(h.entries().len(), MAX_ENTRIES);
        assert_eq!(
            h.entries()[0].url,
            format!("http://p{}.i2p/", MAX_ENTRIES + 4)
        );
    }

    #[test]
    fn titles() {
        let mut h = History::default();
        h.visit("http://a.i2p/", "", 1);
        assert!(h.set_title("http://a.i2p/", "A"));
        assert!(!h.set_title("http://a.i2p/", "A"));
        assert!(!h.set_title("http://b.i2p/", "B"));
    }

    #[test]
    fn query_filters() {
        let mut h = History::default();
        h.visit("http://stats.i2p/", "Stats", 10);
        h.visit("http://forum.i2p/", "Forum STATS", 20);
        h.visit("http://reg.i2p/", "Reg", 30);
        assert_eq!(h.query(&q(None, None, None)).len(), 3);
        assert_eq!(h.query(&q(Some("stats"), None, None)).len(), 2);
        assert_eq!(h.query(&q(Some("stats"), Some(20), None)).len(), 1);
        assert_eq!(h.query(&q(None, None, Some(1)))[0].url, "http://reg.i2p/");
        let first = &h.query(&q(None, None, Some(1)))[0];
        let next = HistoryQuery {
            q: None,
            before: Some(Cursor::Entry {
                visited: first.visited,
                id: first.id.clone(),
            }),
            limit: Some(5),
        };
        let page: Vec<String> = h.query(&next).into_iter().map(|e| e.url).collect();
        assert_eq!(page, ["http://forum.i2p/", "http://stats.i2p/"]);
    }

    #[test]
    fn remove_and_clear() {
        let mut h = History::default();
        h.visit("http://a.i2p/", "", 0);
        h.visit("http://b.i2p/", "", 10 * HOUR_MS);
        let id = h.entries()[0].id.clone();
        assert!(h.remove(&id));
        assert!(!h.remove(&id));
        h.visit("http://c.i2p/", "", 10 * HOUR_MS);
        h.clear(range_ms("hour").unwrap(), 10 * HOUR_MS + 1);
        assert_eq!(h.entries().len(), 1);
        h.clear(range_ms("all").unwrap(), 0);
        assert!(h.entries().is_empty());
    }

    #[test]
    fn ranges() {
        assert_eq!(range_ms("day"), Ok(Some(24 * HOUR_MS)));
        assert_eq!(range_ms("week"), Ok(Some(168 * HOUR_MS)));
        assert!(range_ms("year").is_err());
    }

    #[test]
    fn a_hand_edited_file_is_capped_and_checked() {
        let entry = |i: usize, url: String| HistoryEntry {
            id: i.to_string(),
            url,
            title: String::new(),
            visited: i as u64,
            visits: 1,
        };
        let mut entries: Vec<HistoryEntry> = (0..12_000)
            .map(|i| entry(i, format!("http://p{i}.i2p/")))
            .collect();
        entries.push(entry(99_999, "http://example.com/".into()));
        let h = History::checked(entries);
        assert_eq!(h.entries().len(), MAX_ENTRIES);
        assert_eq!(h.entries()[0].url, "http://p11999.i2p/");
        assert!(h.entries().iter().all(|e| e.url.contains(".i2p")));
    }

    #[test]
    fn load_and_save() {
        let dir = testdir::fresh("history");
        let path = dir.join("history.json");
        assert!(History::load(&path).entries().is_empty());
        let mut h = History::default();
        h.visit("http://a.i2p/", "A", 1);
        h.save(&path).unwrap();
        let back = History::load(&path);
        assert_eq!(back.entries(), h.entries());
    }
}
