// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Session counters for per-request results (R12). A site can make any number of requests,
//! so they are counted, never recorded one by one, and a report shows only a coarse bucket.

use std::sync::{Mutex, MutexGuard, PoisonError};

use super::types::{Code, Field};

static COUNTERS: Mutex<Vec<(Code, Field, u64)>> = Mutex::new(Vec::new());

fn counters_lock() -> MutexGuard<'static, Vec<(Code, Field, u64)>> {
    COUNTERS.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Adds 1 to the session counter of `code` and `field`, saturating.
pub fn count(code: Code, field: Field) {
    let mut all = counters_lock();
    match all.iter_mut().find(|(c, f, _)| *c == code && *f == field) {
        Some(entry) => entry.2 = entry.2.saturating_add(1),
        None => all.push((code, field, 1)),
    }
}

/// The counters above 0, sorted by code name, then field text.
#[must_use]
pub fn counters() -> Vec<(Code, Field, u64)> {
    let mut all: Vec<_> = counters_lock()
        .iter()
        .copied()
        .filter(|(_, _, n)| *n > 0)
        .collect();
    all.sort_by_cached_key(|(c, f, _)| (c.as_str(), f.to_string()));
    all
}

/// `0`, `1+`, `10+`, `100+` or `1000+`.
#[must_use]
pub fn count_bucket(n: u64) -> &'static str {
    match n {
        0 => "0",
        1..10 => "1+",
        10..100 => "10+",
        100..1000 => "100+",
        _ => "1000+",
    }
}

/// `<code> <key>=<value> count=<bucket>` per counter.
#[must_use]
pub fn counter_lines() -> Vec<String> {
    counters()
        .iter()
        .map(|(c, f, n)| format!("{} {f} count={}", c.as_str(), count_bucket(*n)))
        .collect()
}

/// Drops every counter.
pub fn clear() {
    counters_lock().clear();
}
