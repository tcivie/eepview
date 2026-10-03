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

/// A main-frame load event of a tab.
pub fn page(tab: u32, started: bool, url: &str) {
    if enabled() {
        let what = if started { "started" } else { "finished" };
        eprintln!("[eepview] +{}ms tab {tab} {what} {url}", elapsed_ms());
    }
}

/// A router status line.
pub fn router(state: &str, detail: Option<&str>) {
    if enabled() {
        eprintln!(
            "[eepview] +{}ms router {state} {}",
            elapsed_ms(),
            detail.unwrap_or("")
        );
    }
}

/// An error that the user cannot act on.
pub fn error(what: &str, detail: &str) {
    eprintln!("[eepview] {what}: {detail}");
}

/// A layout decision.
pub fn view(what: &str) {
    if enabled() {
        eprintln!("[eepview] +{}ms view {what}", elapsed_ms());
    }
}
