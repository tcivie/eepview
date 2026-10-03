// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The router watcher: VERIFY every 5 s, run the gatekeeper only while VERIFY passes.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Runtime};

use super::apply::with_core;
use super::log;
use super::state::{lock, now_ms, shared};
use crate::core::router::status_of;
use crate::net::gatekeeper::Gatekeeper;
use crate::net::loopback::LoopbackAddr;
use crate::net::verify::{Verdict, verify};

const PERIOD: Duration = Duration::from_secs(5);

/// Starts the watcher thread for the router proxy `proxy` (or the parse error of
/// `EEPVIEW_PROXY`, which keeps the router "down" forever: fail closed).
pub fn start<R: Runtime>(app: &AppHandle<R>, proxy: Result<LoopbackAddr, String>) {
    let app = app.clone();
    spawn("router-watch", "router watcher", move || {
        loop {
            tick(&app, &proxy, super::env::router_helper());
            thread::sleep(PERIOD);
        }
    });
}

/// One round of the watcher: VERIFY, then a statistics sample.
fn tick<R: Runtime>(
    app: &AppHandle<R>,
    proxy: &Result<LoopbackAddr, String>,
    helper: Option<(LoopbackAddr, String)>,
) {
    let verdict = proxy.clone().map_or_else(Verdict::Down, verify);
    apply(app, &verdict);
    sample(app, helper);
}

/// Records the router bandwidth for `router_stats().history`, when a helper is configured.
fn sample<R: Runtime>(app: &AppHandle<R>, helper: Option<(LoopbackAddr, String)>) {
    if let Some((addr, token)) = helper {
        let stats = crate::net::stats::fetch(addr, &token);
        lock(&shared(app).core).record_stats(now_ms(), &stats);
    }
}

/// VERIFY now, off the main thread (after `connection_resume()`).
pub fn check_now<R: Runtime>(app: &AppHandle<R>, proxy: Result<LoopbackAddr, String>) {
    let app = app.clone();
    spawn("router-check", "router check", move || {
        let verdict = proxy.map_or_else(Verdict::Down, verify);
        apply(&app, &verdict);
    });
}

/// Runs `f` on a named thread; `what` names it in the error line.
fn spawn(name: &str, what: &str, f: impl FnOnce() + Send + 'static) {
    if let Err(e) = thread::Builder::new().name(name.into()).spawn(f) {
        log::error(what, &e.to_string());
    }
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
    let proxy = lock(&shared(app).core).router().proxy.clone();
    let status = status_of(verdict, &proxy, gate_ok);
    log::router(status.state, status.detail.as_deref());
    with_core(app, |core| core.router_changed(status));
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
            log::error("gatekeeper", &e.to_string());
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
            Some((dead_addr(), "token".into())),
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
        spawn("test-spawn", "test", move || tx.send(7).unwrap());
        assert_eq!(rx.recv().unwrap(), 7);
    }
}
