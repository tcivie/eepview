// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The router statistics sampler (docs/wiki/router-console.md R47–R53): one thread that
//! takes the router figures every 5 s, whether or not a page shows them, and keeps the
//! bandwidth history. Pages only read what it stored (`router_stats()`).

use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Runtime};

use super::apply::with_core;
use super::console::{current, stopped};
use super::state::{lock, now_ms, shared};
use crate::diag::{self, Code, ErrorKind, Field, OpKind};
use crate::net::console::fetch_stats;
use crate::net::loopback::LoopbackAddr;
use crate::net::stats::{RouterStats, StatsSource, pick, try_fetch};

/// The time between two rounds.
pub const SAMPLE_EVERY: Duration = Duration::from_secs(5);

/// Starts the `stats-sampler` thread unless one runs. True when it started one.
pub fn start<R: Runtime>(app: &AppHandle<R>) -> bool {
    let state = shared(app);
    if state.sampler_live.swap(true, Ordering::SeqCst) {
        return false;
    }
    let (wake, rounds) = mpsc::channel();
    *lock(&state.sampler) = Some(wake);
    let handle = app.clone();
    let spawned = thread::Builder::new()
        .name("stats-sampler".into())
        .spawn(move || {
            run(&handle, &rounds);
            shared(&handle).sampler_live.store(false, Ordering::SeqCst);
        });
    if let Err(e) = spawned {
        lock(&state.sampler).take();
        state.sampler_live.store(false, Ordering::SeqCst);
        diag::event(
            Code::ThreadFailed,
            &[Field::Op(OpKind::Spawn), Field::Error(ErrorKind::from(&e))],
        );
        return false;
    }
    true
}

/// True from a [`start`] that returned true until that thread ends.
#[must_use]
pub fn running<R: Runtime>(app: &AppHandle<R>) -> bool {
    shared(app).sampler_live.load(Ordering::SeqCst)
}

/// Makes the running sampler do a round now. Nothing without a sampler.
pub fn wake<R: Runtime>(app: &AppHandle<R>) {
    if let Some(wake) = lock(&shared(app).sampler).as_ref() {
        let _ = wake.send(());
    }
}

/// Ends the running sampler: it starts no new round, and its thread ends.
pub fn stop<R: Runtime>(app: &AppHandle<R>) {
    lock(&shared(app).sampler).take();
}

/// On quit: ends the console loops and the sampler, then saves the history once.
pub fn shutdown<R: Runtime>(app: &AppHandle<R>) {
    super::console::stop(app);
    stop(app);
    lock(&shared(app).core).save_stats(now_ms());
}

/// The sampler loop: a round at once, then one per [`SAMPLE_EVERY`] or per wake, until
/// [`stop`] drops the wake channel.
fn run<R: Runtime>(app: &AppHandle<R>, rounds: &Receiver<()>) {
    while open(rounds) {
        tick(app, super::env::router_helper());
        if let Err(RecvTimeoutError::Disconnected) = rounds.recv_timeout(SAMPLE_EVERY) {
            return;
        }
    }
}

/// Drops the pending wakes. False once the sampler is stopped.
fn open(rounds: &Receiver<()>) -> bool {
    loop {
        match rounds.try_recv() {
            Ok(()) => {}
            Err(TryRecvError::Empty) => return true,
            Err(TryRecvError::Disconnected) => return false,
        }
    }
}

/// One round. With no gatekeeper (router not verified, or paused): no request, no sample,
/// and the latest figures become all `null`. Else the figures of the helper, else of the
/// stored console (not after `console::stop`): kept as the latest figures, their bandwidth
/// added to the history, and the history saved when a save is due.
pub fn tick<R: Runtime>(app: &AppHandle<R>, helper: Option<(LoopbackAddr, String)>) {
    let shell = shared(app);
    if lock(&shell.gate).is_none() {
        lock(&shell.core).set_latest_stats(RouterStats::default());
        return;
    }
    let (stats, source) = figures(app, helper);
    let version = stats.version.clone();
    let now = now_ms();
    {
        let mut core = lock(&shell.core);
        if source != StatsSource::None {
            core.record_stats_spaced(now, &stats);
        }
        core.set_latest_stats(stats);
        core.save_stats_if_due(now);
    }
    if source == StatsSource::Helper {
        with_core(app, |core| core.router_version(version.as_deref()));
    }
}

/// The figures of the first source that answers: the helper, else the stored console.
fn figures<R: Runtime>(
    app: &AppHandle<R>,
    helper: Option<(LoopbackAddr, String)>,
) -> (RouterStats, StatsSource) {
    let from_helper = helper.and_then(|(addr, token)| try_fetch(addr, &token));
    pick(from_helper, || {
        current(app)
            .filter(|_| !stopped(app))
            .and_then(|console| fetch_stats(&console))
    })
}

#[cfg(test)]
mod tests;
