// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The router watcher: VERIFY every 5 s, run the gatekeeper only while VERIFY passes.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Runtime};

use super::apply::with_core;
use super::state::{lock, now_ms, shared};
use crate::core::Core;
use crate::core::router::status_of;
use crate::diag::{self, Code, ErrorKind, Field, OpKind, RouterState};
use crate::net::gatekeeper::Gatekeeper;
use crate::net::loopback::LoopbackAddr;
use crate::net::verify::{Verdict, verify};
use crate::types::RouterStatus;

const PERIOD: Duration = Duration::from_secs(5);

/// Starts the watcher thread for the router proxy `proxy` (or the parse error of
/// `EEPVIEW_PROXY`, which keeps the router "down" forever: fail closed).
pub fn start<R: Runtime>(app: &AppHandle<R>, proxy: Result<LoopbackAddr, String>) {
    let app = app.clone();
    spawn("router-watch", move || {
        loop {
            tick(&app, &proxy, super::env::router_helper().as_ref());
            thread::sleep(PERIOD);
        }
    });
}

/// One round of the watcher: VERIFY, a statistics sample, then router checks 2 to 4.
fn tick<R: Runtime>(
    app: &AppHandle<R>,
    proxy: &Result<LoopbackAddr, String>,
    helper: Option<&(LoopbackAddr, String)>,
) {
    with_core(app, Core::verify_started);
    let verdict = proxy.clone().map_or_else(Verdict::Down, verify);
    apply(app, &verdict);
    after_verify(app, helper, true);
}

/// After a VERIFY round: the helper statistics, the router version they give, then router
/// checks 2 to 4. With `record`, a configured helper also adds its bandwidth sample to
/// `router_stats().history`.
fn after_verify<R: Runtime>(
    app: &AppHandle<R>,
    helper: Option<&(LoopbackAddr, String)>,
    record: bool,
) {
    let answer = helper.and_then(|(addr, token)| crate::net::stats::try_fetch(*addr, token));
    if record && helper.is_some() {
        let stats = answer.clone().unwrap_or_default();
        lock(&shared(app).core).record_stats(now_ms(), &stats);
    }
    let version = answer.as_ref().and_then(|s| s.version.clone());
    with_core(app, |core| core.router_version(version.as_deref()));
    super::checks::run(app, answer);
}

/// VERIFY now, off the main thread (after `connection_resume()`).
pub fn check_now<R: Runtime>(app: &AppHandle<R>, proxy: Result<LoopbackAddr, String>) {
    let app = app.clone();
    spawn("router-check", move || {
        with_core(&app, Core::verify_started);
        let verdict = proxy.map_or_else(Verdict::Down, verify);
        apply(&app, &verdict);
        after_verify(&app, super::env::router_helper().as_ref(), false);
    });
}

/// Runs `f` on a named thread. False when the thread could not start.
pub(super) fn spawn(name: &str, f: impl FnOnce() + Send + 'static) -> bool {
    let started = thread::Builder::new().name(name.into()).spawn(f);
    if let Err(e) = &started {
        diag::event(
            Code::ThreadFailed,
            &[Field::Op(OpKind::Spawn), Field::Error(ErrorKind::from(e))],
        );
    }
    started.is_ok()
}

/// Closes the gatekeeper (after `connection_pause()`).
pub fn close_gate<R: Runtime>(app: &AppHandle<R>) {
    lock(&shared(app).gate).take().inspect(|g| g.close());
}

/// Starts or stops the gatekeeper for a verdict, then tells the core.
fn apply<R: Runtime>(app: &AppHandle<R>, verdict: &Verdict) {
    let gate_ok = if let Verdict::Ok(upstream) = verdict {
        ensure_gate(app, upstream)
    } else {
        close_gate(app);
        false
    };
    let before = lock(&shared(app).core).router().clone();
    let status = status_of(verdict, &before.proxy, gate_ok);
    record(&before, matches!(verdict, Verdict::Ok(_)), &status);
    with_core(app, |core| core.router_checked(now_ms(), status));
}

/// Records a change of the router state: the VERIFY result, then up or down.
fn record(before: &RouterStatus, verified: bool, after: &RouterStatus) {
    if before.state == after.state {
        return;
    }
    let state = Field::Router(RouterState::from_contract(after.state));
    if verified {
        diag::event(Code::VerifyPassed, &[]);
    } else {
        diag::event(Code::VerifyFailed, &[state]);
    }
    let now_ok = after.state == "ok" && !before.paused;
    if now_ok && !before.is_ok() {
        diag::event(Code::RouterUp, &[state]);
    } else if before.is_ok() && !now_ok {
        diag::event(Code::RouterDown, &[state]);
    }
}

fn ensure_gate<R: Runtime>(
    app: &AppHandle<R>,
    upstream: &crate::net::verify::VerifiedUpstream,
) -> bool {
    let state = shared(app);
    let mut gate = lock(&state.gate);
    // Lock order gate, then core. A pause sets the core flag first and closes the gate
    // second, so a gate opened here before the pause is closed by it.
    if lock(&state.core).paused() {
        return false;
    }
    if gate.is_some() {
        return true;
    }
    match Gatekeeper::start(upstream) {
        Ok(g) => {
            *gate = Some(Arc::new(g));
            true
        }
        Err(e) => {
            diag::event(
                Code::GatekeeperStartFailed,
                &[Field::Error(ErrorKind::from(&e))],
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::testing::{FakeRouter, dead_addr};
    use crate::shell::testing::{app, core, gate_open, wait_for};

    #[test]
    fn a_passing_verify_opens_the_gate() {
        let app = app();
        let router = FakeRouter::start();
        tick(app.handle(), &Ok(router.addr), None);
        assert!(gate_open(&app));
        assert_eq!(core(&app).router().state, "ok");
        tick(
            app.handle(),
            &Ok(router.addr),
            Some(&(dead_addr(), "token".into())),
        );
        assert!(gate_open(&app));
        assert!(core(&app).stats_history().is_empty());
    }

    #[test]
    fn a_failing_verify_closes_the_gate() {
        let app = app();
        let router = FakeRouter::start();
        tick(app.handle(), &Ok(router.addr), None);
        tick(app.handle(), &Ok(dead_addr()), None);
        assert!(!gate_open(&app));
        assert_eq!(core(&app).router().state, "down");
        tick(app.handle(), &Err("EEPVIEW_PROXY: bad".into()), None);
        assert!(!gate_open(&app));
    }

    #[test]
    fn a_paused_connection_keeps_the_gate_closed() {
        let app = app();
        let router = FakeRouter::start();
        core(&app).pause();
        tick(app.handle(), &Ok(router.addr), None);
        assert!(!gate_open(&app));
        close_gate(app.handle());
        assert!(!gate_open(&app));
    }

    #[test]
    fn the_watcher_and_the_check_run_on_their_threads() {
        let app = app();
        start(app.handle(), Err("EEPVIEW_PROXY: bad".into()));
        assert!(wait_for(|| core(&app).router().state == "down"));
        let router = FakeRouter::start();
        check_now(app.handle(), Ok(router.addr));
        assert!(wait_for(|| gate_open(&app)));
    }

    #[test]
    fn spawn_runs_the_closure() {
        let (tx, rx) = std::sync::mpsc::channel();
        assert!(spawn("test-spawn", move || tx.send(7).unwrap()));
        assert_eq!(rx.recv().unwrap(), 7);
    }
}
