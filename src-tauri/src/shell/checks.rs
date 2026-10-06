// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Router checks 2 to 4 (`docs/wiki/router-checks.md`, V9, V10): the facts of one VERIFY
//! round, read while the gate is open, then judged by the core.

use tauri::{AppHandle, Runtime};

use super::apply::with_core;
use super::console::{current, stopped};
use super::state::{lock, now_ms, shared};
use crate::core::checks::CheckFacts;
use crate::net::console::{VerifiedConsole, fetch_stats};
use crate::net::outproxy::{OutproxyFinding, find_outproxy};
use crate::net::stats::RouterStats;

/// Runs checks 2 to 4 for this round, with the statistics the router helper gave (`None`
/// when it did not answer). Does nothing while the gate is closed or paused.
pub fn run<R: Runtime>(app: &AppHandle<R>, helper: Option<RouterStats>) {
    let router = lock(&shared(app).core).router().clone();
    let (open, version, proxy) = (router.is_ok(), router.version, router.proxy);
    if !open {
        return;
    }
    let console = current(app).filter(|_| !stopped(app));
    let env = |name: &str| std::env::var(name).ok();
    let facts = gather(console.as_ref(), helper, version, &proxy, &env);
    with_core(app, |core| core.checks_seen(now_ms(), &facts));
}

/// The facts of V10: the router type and version, the statistics (helper first, else one
/// console request) and the outproxy configuration on the proxy port.
#[must_use]
pub fn gather(
    console: Option<&VerifiedConsole>,
    helper: Option<RouterStats>,
    version: Option<String>,
    proxy: &str,
    env: &dyn Fn(&str) -> Option<String>,
) -> CheckFacts {
    let kind = console.map(VerifiedConsole::kind);
    let stats = helper.or_else(|| console.and_then(fetch_stats));
    let version = version.or_else(|| console.and_then(|c| c.version().map(str::to_owned)));
    let outproxy = proxy_port(proxy).map_or_else(
        || OutproxyFinding::Unknown(format!("The router proxy {proxy} names no port.")),
        |port| find_outproxy(kind, port, env),
    );
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
    use crate::shell::testing::{app, core};
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

    fn state_of(app: &tauri::App<crate::shell::testing::Mock>, id: CheckId) -> CheckState {
        let core = core(app);
        core.checks().iter().find(|c| c.id == id).unwrap().state
    }

    #[test]
    fn nothing_runs_while_the_gate_is_closed() {
        let app = app();
        run(app.handle(), None);
        assert!(
            core(&app)
                .checks()
                .iter()
                .all(|c| c.state == CheckState::Pending)
        );
    }

    #[test]
    fn a_round_reads_the_console_type_and_version() {
        let app = app();
        let fake = FakeConsole::start(ConsoleKind::Java);
        set_console(app.handle(), Some(fake.verified()));
        let _ = core(&app).router_checked(1, ok_status());
        run(app.handle(), None);
        assert_eq!(state_of(&app, CheckId::Version), CheckState::Passed);
        assert_ne!(state_of(&app, CheckId::Tunnels), CheckState::Running);
        assert_ne!(state_of(&app, CheckId::NoOutproxy), CheckState::Running);
    }

    #[test]
    fn the_helper_statistics_come_first() {
        let empty = |_: &str| None;
        let stats = RouterStats {
            network_status: Some("OK".into()),
            ..RouterStats::default()
        };
        let facts = gather(None, Some(stats.clone()), None, "127.0.0.1:4444", &empty);
        assert_eq!(facts.stats, Some(stats));
        assert_eq!(facts.kind, None);
        assert!(matches!(facts.outproxy, OutproxyFinding::Unknown(_)));
    }

    #[test]
    fn a_proxy_with_no_port_is_not_checked() {
        let empty = |_: &str| None;
        let facts = gather(None, None, None, "proxy", &empty);
        assert!(matches!(facts.outproxy, OutproxyFinding::Unknown(r) if r.contains("no port")));
        assert_eq!(proxy_port("127.0.0.1:4445"), Some(4445));
        assert_eq!(proxy_port("127.0.0.1:x"), None);
    }
}
