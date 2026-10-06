// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests for a failed main-frame load (`Core::load_failed`). The source is the
//! "Failed loads" section of `docs/wiki/browser-shell.md` (F1, F5, F6). The tests read the
//! requirement and the public interface only, never the implementation.

use eepview_lib::core::Core;
use eepview_lib::nav::{INTERNAL_PAGES, Target, classify};
use eepview_lib::session::Step;
use eepview_lib::types::{HistoryQuery, RouterStatus};
use tauri::Url;

const PAGE_A: &str = "http://a.i2p/";
const PAGE_B: &str = "http://b.i2p/";
const FAILED: &str = "http://a.i2p/x";
const CODE: &str = "NSURLErrorDomain -1022";
const NOW: u64 = 10;

fn ok_status() -> RouterStatus {
    RouterStatus {
        state: "ok",
        proxy: "127.0.0.1:4444".into(),
        version: None,
        detail: None,
        paused: false,
        managed: false,
    }
}

/// A core with a working router and the active tab loading `url`.
fn loading(url: &str) -> (Core, u32) {
    let mut core = Core::new(None, "127.0.0.1:4444", 0);
    core.router_changed(ok_status());
    core.navigate(url);
    let id = core.tabs().active_id();
    core.page_started(id, url);
    (core, id)
}

/// The url of tab `id`, or an empty text when there is no such tab.
fn url_of(core: &Core, id: u32) -> String {
    core.tab_info(id).map(|t| t.url).unwrap_or_default()
}

fn is_loading(core: &Core, id: u32) -> bool {
    core.tab_info(id).is_some_and(|t| t.nav.loading)
}

/// The query pairs of an error page address, or none when it is not one.
fn pairs(address: &str) -> Vec<(String, String)> {
    let Ok(url) = Url::parse(address) else {
        return Vec::new();
    };
    if url.scheme() != "eepview" || url.host_str() != Some("load-failed") {
        return Vec::new();
    }
    url.query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

/// The value of the query pair `key`, if it is there.
fn pair(address: &str, key: &str) -> Option<String> {
    pairs(address)
        .into_iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
}

fn history_len(core: &Core) -> usize {
    core.history_query(&HistoryQuery::default()).len()
}

// ---------------------------------------------------------------------------------------------
// F1: the failed load shows the error page
// ---------------------------------------------------------------------------------------------

#[test]
fn f1_a_failed_load_shows_the_load_failed_page() {
    let (mut core, id) = loading(PAGE_A);
    core.load_failed(id, Some(FAILED), "blocked", CODE);
    let shown = url_of(&core, id);
    assert!(
        shown.starts_with("eepview://load-failed?"),
        "F1: shows {shown}"
    );
}

#[test]
fn f1_f5_the_page_address_carries_url_reason_and_code() {
    let (mut core, id) = loading(PAGE_A);
    core.load_failed(id, Some(FAILED), "blocked", CODE);
    let shown = url_of(&core, id);
    assert_eq!(pair(&shown, "url").as_deref(), Some(FAILED), "F5 url");
    assert_eq!(
        pair(&shown, "reason").as_deref(),
        Some("blocked"),
        "F1 reason"
    );
    assert_eq!(pair(&shown, "code").as_deref(), Some(CODE), "F1 code");
}

#[test]
fn f1_the_reason_and_code_are_passed_on_as_given() {
    let (mut core, id) = loading(PAGE_A);
    core.load_failed(id, Some(FAILED), "unreachable", "NSURLErrorDomain -1003");
    let shown = url_of(&core, id);
    assert_eq!(pair(&shown, "reason").as_deref(), Some("unreachable"));
    assert_eq!(
        pair(&shown, "code").as_deref(),
        Some("NSURLErrorDomain -1003")
    );
}

#[test]
fn f1_the_tab_stops_loading() {
    let (mut core, id) = loading(PAGE_A);
    assert!(
        is_loading(&core, id),
        "F1: the tab loads before the failure"
    );
    core.load_failed(id, Some(FAILED), "engine", CODE);
    assert!(!is_loading(&core, id), "F1: a tab never spins forever");
}

#[test]
fn f1_a_failure_after_the_first_commit_shows_the_page_too() {
    let (mut core, id) = loading(PAGE_A);
    core.page_finished(id, PAGE_A, NOW);
    core.reload(false);
    core.page_started(id, PAGE_A);
    core.load_failed(id, Some(PAGE_A), "unreachable", "NSURLErrorDomain -1005");
    assert!(url_of(&core, id).starts_with("eepview://load-failed?"));
    assert!(!is_loading(&core, id));
}

#[test]
fn f1_the_failure_returns_effects_that_update_the_tab() {
    let (mut core, id) = loading(PAGE_A);
    let effects = core.load_failed(id, Some(FAILED), "blocked", CODE);
    assert!(
        !effects.is_empty(),
        "F1: the shell must learn of the change"
    );
}

#[test]
fn f1_a_failure_of_a_background_tab_shows_the_page_in_that_tab() {
    let (mut core, first) = loading(PAGE_A);
    core.tab_new(None, eepview_lib::tabs::Place::End);
    let second = core.tabs().active_id();
    assert_ne!(first, second);
    core.load_failed(first, Some(FAILED), "blocked", CODE);
    assert!(url_of(&core, first).starts_with("eepview://load-failed?"));
    assert_eq!(
        url_of(&core, second),
        core.home_url(),
        "the other tab stays"
    );
}

// ---------------------------------------------------------------------------------------------
// F5: the address on the page
// ---------------------------------------------------------------------------------------------

#[test]
fn f5_no_failed_address_gives_the_address_the_tab_was_loading() {
    let (mut core, id) = loading(PAGE_A);
    core.load_failed(id, None, "engine", CODE);
    assert_eq!(pair(&url_of(&core, id), "url").as_deref(), Some(PAGE_A));
}

#[test]
fn f5_a_non_i2p_address_never_reaches_the_page() {
    let (mut core, id) = loading(PAGE_A);
    core.load_failed(id, Some("http://example.com/"), "blocked", CODE);
    let shown = url_of(&core, id);
    assert_eq!(
        pair(&shown, "url").as_deref(),
        Some(PAGE_A),
        "F5: the tab address"
    );
    assert!(
        pairs(&shown)
            .iter()
            .all(|(_, v)| !v.contains("example.com")),
        "F5: leaked in {shown}"
    );
    assert!(!shown.contains("example.com"), "F5: leaked in {shown}");
}

#[test]
fn f5_other_clearnet_forms_never_reach_the_page() {
    for bad in [
        "https://example.com/path?q=1",
        "http://127.0.0.1:7657/",
        "ftp://files.example.org/",
        "file:///etc/passwd",
        "not a url",
        "",
    ] {
        let (mut core, id) = loading(PAGE_A);
        core.load_failed(id, Some(bad), "blocked", CODE);
        let shown = url_of(&core, id);
        assert_eq!(
            pair(&shown, "url").as_deref(),
            Some(PAGE_A),
            "failed {bad:?}"
        );
    }
}

#[test]
fn f5_another_allowed_i2p_address_is_used() {
    let (mut core, id) = loading(PAGE_A);
    core.load_failed(id, Some("http://other.i2p/deep?x=1"), "engine", CODE);
    let shown = url_of(&core, id);
    assert_eq!(
        pair(&shown, "url").as_deref(),
        Some("http://other.i2p/deep?x=1")
    );
}

#[test]
fn f5_the_address_with_special_characters_survives_the_query() {
    let (mut core, id) = loading(PAGE_A);
    let tricky = "http://a.i2p/p?x=1&y=2#frag";
    core.load_failed(id, Some(tricky), "engine", CODE);
    let shown = url_of(&core, id);
    assert_eq!(
        pair(&shown, "reason").as_deref(),
        Some("engine"),
        "no pair leaks"
    );
    assert_eq!(pair(&shown, "code").as_deref(), Some(CODE), "no pair leaks");
    assert_eq!(pair(&shown, "url").as_deref(), Some(tricky));
}

// ---------------------------------------------------------------------------------------------
// F6: history, back, and failures that change nothing
// ---------------------------------------------------------------------------------------------

#[test]
fn f6_a_failed_load_makes_no_history_entry() {
    let (mut core, id) = loading(PAGE_A);
    assert_eq!(history_len(&core), 0, "nothing is visited yet");
    core.load_failed(id, Some(FAILED), "blocked", CODE);
    assert_eq!(history_len(&core), 0, "F6: no history entry");
}

#[test]
fn f6_the_error_page_adds_no_entry_after_a_good_visit() {
    let (mut core, id) = loading(PAGE_A);
    core.page_finished(id, PAGE_A, NOW);
    let before = history_len(&core);
    core.navigate(PAGE_B);
    core.page_started(id, PAGE_B);
    core.load_failed(id, Some(PAGE_B), "unreachable", "NSURLErrorDomain -1004");
    assert_eq!(history_len(&core), before, "F6: only the good visit stays");
    let entries = core.history_query(&HistoryQuery::default());
    assert!(entries.iter().all(|e| !e.url.starts_with("eepview://")));
    assert!(entries.iter().all(|e| e.url != PAGE_B));
}

#[test]
fn f6_back_is_possible_from_the_error_page() {
    let (mut core, id) = loading(PAGE_A);
    core.page_finished(id, PAGE_A, NOW);
    core.navigate(PAGE_B);
    core.page_started(id, PAGE_B);
    core.load_failed(id, Some(PAGE_B), "unreachable", "NSURLErrorDomain -1004");
    let can_back = core.tab_info(id).is_some_and(|t| t.nav.can_back);
    assert!(can_back, "F6: back leaves the error page");
}

#[test]
fn f6_back_leaves_the_error_page_for_the_page_before_it() {
    let (mut core, id) = loading(PAGE_A);
    core.page_finished(id, PAGE_A, NOW);
    core.navigate(PAGE_B);
    core.page_started(id, PAGE_B);
    core.load_failed(id, Some(PAGE_B), "unreachable", "NSURLErrorDomain -1004");
    let effects = core.step(Step::Back);
    assert!(!effects.is_empty(), "F6: back does something");
    core.page_started(id, PAGE_A);
    assert_eq!(
        url_of(&core, id),
        PAGE_A,
        "F6: the page before the error page"
    );
}

#[test]
fn f6_a_failure_of_a_tab_showing_an_internal_page_changes_nothing() {
    let mut core = Core::new(None, "127.0.0.1:4444", 0);
    core.router_changed(ok_status());
    let id = core.tabs().active_id();
    let before = url_of(&core, id);
    let effects = core.load_failed(id, Some(PAGE_A), "blocked", CODE);
    assert!(effects.is_empty(), "F6: no effects, got {effects:?}");
    assert_eq!(url_of(&core, id), before, "F6: the url is unchanged");
}

#[test]
fn f6_a_failure_for_an_unknown_tab_changes_nothing() {
    let (mut core, id) = loading(PAGE_A);
    let effects = core.load_failed(id + 1000, Some(FAILED), "blocked", CODE);
    assert!(effects.is_empty(), "F6: no effects, got {effects:?}");
    assert_eq!(url_of(&core, id), PAGE_A);
    assert!(is_loading(&core, id), "the real tab keeps loading");
}

#[test]
fn f6_a_second_failure_after_the_first_changes_nothing() {
    let (mut core, id) = loading(PAGE_A);
    core.load_failed(id, Some(FAILED), "blocked", CODE);
    let shown = url_of(&core, id);
    let effects = core.load_failed(id, Some(PAGE_B), "unreachable", "NSURLErrorDomain -1003");
    assert!(effects.is_empty(), "F6: no effects, got {effects:?}");
    assert_eq!(url_of(&core, id), shown, "F6: the first error page stays");
}

#[test]
fn f6_a_failure_after_the_load_finished_changes_nothing() {
    let (mut core, id) = loading(PAGE_A);
    core.page_finished(id, PAGE_A, NOW);
    let effects = core.load_failed(id, Some(PAGE_A), "engine", CODE);
    assert!(
        effects.is_empty(),
        "F6: the load already ended, got {effects:?}"
    );
    assert_eq!(url_of(&core, id), PAGE_A);
}

// ---------------------------------------------------------------------------------------------
// The page is an internal page
// ---------------------------------------------------------------------------------------------

#[test]
fn f1_load_failed_is_an_internal_page() {
    assert!(INTERNAL_PAGES.contains(&"load-failed"), "F1: internal page");
}

#[test]
fn f1_the_load_failed_address_classifies_as_internal() {
    let target = classify("eepview://load-failed?url=x");
    assert!(
        matches!(&target, Target::Internal(page) if page == "eepview://load-failed?url=x"),
        "F1: got {target:?}"
    );
}
