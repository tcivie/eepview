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
    let spawned = thread::Builder::new()
        .name("router-watch".into())
        .spawn(move || {
            loop {
                let verdict = proxy.clone().map_or_else(Verdict::Down, verify);
                apply(&app, &verdict);
                sample(&app);
                thread::sleep(PERIOD);
            }
        });
    if let Err(e) = spawned {
        log::error("router watcher", &e.to_string());
    }
}

/// Records the router bandwidth for `router_stats().history`, when a helper is configured.
fn sample<R: Runtime>(app: &AppHandle<R>) {
    if let Some((addr, token)) = super::env::router_helper() {
        let stats = crate::net::stats::fetch(addr, &token);
        lock(&shared(app).core).record_stats(now_ms(), &stats);
    }
}

/// VERIFY now, off the main thread (after `connection_resume()`).
pub fn check_now<R: Runtime>(app: &AppHandle<R>, proxy: Result<LoopbackAddr, String>) {
    let app = app.clone();
    let spawned = thread::Builder::new()
        .name("router-check".into())
        .spawn(move || {
            let verdict = proxy.map_or_else(Verdict::Down, verify);
            apply(&app, &verdict);
        });
    if let Err(e) = spawned {
        log::error("router check", &e.to_string());
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
