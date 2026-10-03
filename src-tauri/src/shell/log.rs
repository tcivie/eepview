// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Diagnostics on stderr. Page-load timing prints only when `EEPVIEW_LOG` is set.

use std::sync::OnceLock;
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();

fn enabled() -> bool {
    std::env::var_os("EEPVIEW_LOG").is_some()
}

/// Marks the start of the run; page timings count from here.
pub fn start() {
    START.get_or_init(Instant::now);
}

fn elapsed_ms() -> u128 {
    START.get_or_init(Instant::now).elapsed().as_millis()
}

/// Prints `line` when `on`.
fn say(on: bool, line: &str) {
    if on {
        eprintln!("{line}");
    }
}

/// A diagnostic line with the time since the start.
fn timed(text: &str) -> String {
    format!("[eepview] +{}ms {text}", elapsed_ms())
}

/// The text of a page-load line.
fn page_text(tab: u32, started: bool, url: &str) -> String {
    let what = if started { "started" } else { "finished" };
    format!("tab {tab} {what} {url}")
}

/// A main-frame load event of a tab.
pub fn page(tab: u32, started: bool, url: &str) {
    say(enabled(), &timed(&page_text(tab, started, url)));
}

/// A router status line.
pub fn router(state: &str, detail: Option<&str>) {
    let text = format!("router {state} {}", detail.unwrap_or(""));
    say(enabled(), &timed(&text));
}

/// An error that the user cannot act on.
pub fn error(what: &str, detail: &str) {
    say(true, &format!("[eepview] {what}: {detail}"));
}

/// A layout decision.
pub fn view(what: &str) {
    say(enabled(), &timed(&format!("view {what}")));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_lines_name_the_event() {
        assert_eq!(
            page_text(2, true, "http://a.i2p/"),
            "tab 2 started http://a.i2p/"
        );
        assert_eq!(page_text(2, false, "u"), "tab 2 finished u");
    }

    #[test]
    fn timed_lines_carry_the_elapsed_time() {
        start();
        let line = timed("x");
        assert!(
            line.starts_with("[eepview] +") && line.ends_with("ms x"),
            "{line}"
        );
    }

    #[test]
    fn every_line_kind_prints() {
        say(true, "[eepview] test line");
        say(false, "never printed");
        page(1, true, "http://a.i2p/");
        router("ok", None);
        router("down", Some("no router"));
        view("test");
        error("test", "detail");
        assert_eq!(enabled(), std::env::var_os("EEPVIEW_LOG").is_some());
    }
}
