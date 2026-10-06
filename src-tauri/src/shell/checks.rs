// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Router checks 2 to 4 (`docs/wiki/router-checks.md`, V9, V10): the facts of one VERIFY
//! round, read while the gate is open, then judged by the core.

use std::sync::atomic::Ordering;

use tauri::{AppHandle, Runtime};

use super::apply::with_core;
use super::console::{current, stopped};
use super::state::{lock, now_ms, shared};
use crate::core::checks::CheckFacts;
use crate::net::console::{VerifiedConsole, fetch_stats};
use crate::net::outproxy::{OutproxyFinding, RouterKind, config_stamps, find_outproxy};
use crate::net::stats::RouterStats;

/// Runs checks 2 to 4 for this round on a thread of their own, with the statistics the
/// router helper gave (`None` when it did not answer). Does nothing while the gate is
/// closed or paused, or while the run of an earlier round still runs (V10).
pub fn run<R: Runtime>(app: &AppHandle<R>, helper: Option<RouterStats>) {
    let router = lock(&shared(app).core).router().clone();
    if !router.is_ok() || shared(app).checks_busy.swap(true, Ordering::SeqCst) {
        return;
    }
    let handle = app.clone();
    let started = super::watch::spawn("router-checks", move || {
        round(&handle, helper, router.version, &router.proxy);
        shared(&handle).checks_busy.store(false, Ordering::SeqCst);
    });
    if !started {
        shared(app).checks_busy.store(false, Ordering::SeqCst);
    }
}

/// One run of checks 2 to 4: read the facts, then let the core judge them.
fn round<R: Runtime>(
    app: &AppHandle<R>,
    helper: Option<RouterStats>,
    version: Option<String>,
    proxy: &str,
) {
    let console = current(app).filter(|_| !stopped(app));
    let env = |name: &str| std::env::var(name).ok();
    let kind = console.as_ref().map(VerifiedConsole::kind);
    let outproxy = outproxy_of(app, kind, proxy, &env);
    let facts = gather(console.as_ref(), helper, version, outproxy);
    with_core(app, |core| core.checks_seen(now_ms(), &facts));
}

/// The outproxy finding for the proxy port. The files are read again only when their list
/// or a modification time changed since the last round (V12).
fn outproxy_of<R: Runtime>(
    app: &AppHandle<R>,
    kind: Option<RouterKind>,
    proxy: &str,
    env: &dyn Fn(&str) -> Option<String>,
) -> OutproxyFinding {
    let Some(port) = proxy_port(proxy) else {
        return OutproxyFinding::Unknown(format!("The router proxy {proxy} names no port."));
    };
    let key = (kind, port, config_stamps(kind, env));
    let state = shared(app);
    let mut seen = lock(&state.outproxy_seen);
    if let Some((_, finding)) = seen.as_ref().filter(|(k, _)| *k == key) {
        return finding.clone();
    }
    let finding = find_outproxy(kind, port, env);
    *seen = Some((key, finding.clone()));
    finding
}

/// The facts of V10: the router type and version, the statistics (helper first, else one
/// console request) and the outproxy finding.
#[must_use]
pub fn gather(
    console: Option<&VerifiedConsole>,
    helper: Option<RouterStats>,
    version: Option<String>,
    outproxy: OutproxyFinding,
) -> CheckFacts {
    let kind = console.map(VerifiedConsole::kind);
    let stats = helper.or_else(|| console.and_then(fetch_stats));
    let version = version.or_else(|| console.and_then(|c| c.version().map(str::to_owned)));
    CheckFacts {
        kind,
        version,
        outproxy,
        stats,
    }
}

/// The port of a `host:port` proxy address.
fn proxy_port(proxy: &str) -> Option<u16> {
    proxy.rsplit_once(':')?.1.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::console::ConsoleKind;
    use crate::net::testing::FakeConsole;
    use crate::shell::console::set_console;
    use crate::shell::testing::{Mock, app, core, wait_for};
    use crate::types::{CheckId, CheckState, RouterStatus};

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

    fn state_of(app: &tauri::App<Mock>, id: CheckId) -> CheckState {
        let core = core(app);
        core.checks().iter().find(|c| c.id == id).unwrap().state
    }

    #[test]
    fn nothing_runs_while_the_gate_is_closed() {
        let app = app();
        run(app.handle(), None);
        assert!(!shared(app.handle()).checks_busy.load(Ordering::SeqCst));
        let checks = core(&app).checks().to_vec();
        assert!(checks.iter().all(|c| c.state == CheckState::Pending));
    }

    #[test]
    fn a_round_reads_the_console_type_and_version_off_the_watcher() {
        let app = app();
        let fake = FakeConsole::start(ConsoleKind::Java);
        set_console(app.handle(), Some(fake.verified()));
        let _ = core(&app).router_checked(1, ok_status());
        run(app.handle(), None);
        assert!(wait_for(
            || state_of(&app, CheckId::Version) == CheckState::Passed
        ));
        assert!(wait_for(|| !shared(app.handle())
            .checks_busy
            .load(Ordering::SeqCst)));
        assert_ne!(state_of(&app, CheckId::Tunnels), CheckState::Running);
        assert_ne!(state_of(&app, CheckId::NoOutproxy), CheckState::Running);
    }

    #[test]
    fn a_busy_run_skips_the_round() {
        let app = app();
        let _ = core(&app).router_checked(1, ok_status());
        shared(app.handle())
            .checks_busy
            .store(true, Ordering::SeqCst);
        run(app.handle(), None);
        assert_eq!(state_of(&app, CheckId::Version), CheckState::Running);
    }

    #[test]
    fn the_outproxy_finding_is_kept_while_the_files_stay() {
        let app = app();
        let empty = |_: &str| None;
        let first = outproxy_of(app.handle(), None, "127.0.0.1:4444", &empty);
        assert!(lock(&shared(app.handle()).outproxy_seen).is_some());
        let again = outproxy_of(app.handle(), None, "127.0.0.1:4444", &empty);
        assert_eq!(first, again);
        let bad = outproxy_of(app.handle(), None, "proxy", &empty);
        assert!(matches!(bad, OutproxyFinding::Unknown(r) if r.contains("no port")));
    }

    #[test]
    fn the_helper_statistics_come_first() {
        let stats = RouterStats {
            network_status: Some("OK".into()),
            ..RouterStats::default()
        };
        let clear = OutproxyFinding::Clear { file: "f".into() };
        let facts = gather(None, Some(stats.clone()), None, clear);
        assert_eq!(facts.stats, Some(stats));
        assert_eq!(facts.kind, None);
        assert_eq!(proxy_port("127.0.0.1:4445"), Some(4445));
        assert_eq!(proxy_port("127.0.0.1:x"), None);
    }
}
