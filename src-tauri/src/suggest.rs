// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Address-bar suggestions from bookmarks and history. Pure ranking.

use crate::types::{Bookmark, HistoryEntry, Suggestion};

/// Most suggestions returned.
pub const MAX_SUGGESTIONS: usize = 8;
const DAY_MS: u64 = 86_400_000;

/// How well a candidate matches the typed text. Higher is better; 0 is no match.
fn match_score(url: &str, title: &str, needle: &str) -> u32 {
    let bare = url
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .to_lowercase();
    let title = title.to_lowercase();
    if bare.starts_with(needle) {
        return 400;
    }
    if bare.split(['.', '/']).any(|part| part.starts_with(needle)) {
        return 300;
    }
    if title.split_whitespace().any(|w| w.starts_with(needle)) {
        return 200;
    }
    if bare.contains(needle) || title.contains(needle) {
        return 100;
    }
    0
}

/// A small boost for visit count and recency.
fn usage_score(entry: &HistoryEntry, now: u64) -> u32 {
    let visits = entry.visits.min(50);
    let age_days = now.saturating_sub(entry.visited) / DAY_MS;
    let recency = 30u32.saturating_sub(u32::try_from(age_days).unwrap_or(u32::MAX));
    visits + recency
}

/// Up to [`MAX_SUGGESTIONS`] matches, best first, one per URL.
#[must_use]
pub fn suggest(
    input: &str,
    bookmarks: &[Bookmark],
    history: &[HistoryEntry],
    now: u64,
) -> Vec<Suggestion> {
    let needle = input.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    let mut ranked: Vec<(u32, Suggestion)> = Vec::new();
    for b in bookmarks {
        let score = match_score(&b.url, &b.title, &needle);
        if score > 0 {
            ranked.push((score + 60, item(&b.url, &b.title, "bookmark")));
        }
    }
    for h in history {
        let score = match_score(&h.url, &h.title, &needle);
        if score > 0 && !ranked.iter().any(|(_, s)| s.url == h.url) {
            ranked.push((
                score + usage_score(h, now),
                item(&h.url, &h.title, "history"),
            ));
        }
    }
    ranked.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    ranked
        .into_iter()
        .take(MAX_SUGGESTIONS)
        .map(|(_, s)| s)
        .collect()
}

fn item(url: &str, title: &str, source: &'static str) -> Suggestion {
    Suggestion {
        url: url.to_owned(),
        title: title.to_owned(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bm(url: &str, title: &str) -> Bookmark {
        Bookmark {
            id: url.into(),
            url: url.into(),
            title: title.into(),
            folder: None,
            created: 0,
        }
    }

    fn visit(url: &str, title: &str, visits: u32, visited: u64) -> HistoryEntry {
        HistoryEntry {
            id: url.into(),
            url: url.into(),
            title: title.into(),
            visited,
            visits,
        }
    }

    #[test]
    fn prefix_beats_substring_and_bookmarks_win_ties() {
        let b = [bm("http://stats.i2p/", "Stats")];
        let h = [
            visit("http://forum.i2p/stats", "Forum", 3, 0),
            visit("http://stats.i2p/", "Stats", 9, 0),
            visit("http://mystats.i2p/", "My", 1, 0),
        ];
        let s = suggest("stats", &b, &h, 0);
        let urls: Vec<&str> = s.iter().map(|x| x.url.as_str()).collect();
        assert_eq!(
            urls,
            [
                "http://stats.i2p/",
                "http://forum.i2p/stats",
                "http://mystats.i2p/"
            ]
        );
        assert_eq!(s[0].source, "bookmark");
        assert_eq!(s[1].source, "history");
    }

    #[test]
    fn title_words_and_usage() {
        let h = [
            visit("http://a.i2p/", "Old news", 1, 0),
            visit("http://b.i2p/", "Fresh news", 20, 100 * DAY_MS),
        ];
        let s = suggest("NEWS", &[], &h, 100 * DAY_MS);
        assert_eq!(s[0].url, "http://b.i2p/");
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn empty_input_and_cap() {
        assert!(suggest("  ", &[bm("http://a.i2p/", "a")], &[], 0).is_empty());
        let h: Vec<HistoryEntry> = (0..20)
            .map(|i| visit(&format!("http://s{i}.i2p/"), "", 1, 0))
            .collect();
        assert_eq!(suggest("s", &[], &h, 0).len(), MAX_SUGGESTIONS);
        assert!(suggest("zzz", &[], &h, 0).is_empty());
    }
}
