//! The router watcher: VERIFY every 5 s, run the gatekeeper only while VERIFY passes.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Runtime};

use super::apply::with_core;
use super::log;
use super::state::{lock, shared};
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
                let verdict = match proxy {
                    Ok(addr) => verify(addr),
                    Err(ref e) => Verdict::Down(e.clone()),
                };
                apply(&app, &verdict);
                thread::sleep(PERIOD);
            }
        });
    if let Err(e) = spawned {
        log::error("router watcher", &e.to_string());
    }
}

/// Starts or stops the gatekeeper for a verdict, then tells the core.
fn apply<R: Runtime>(app: &AppHandle<R>, verdict: &Verdict) {
    let gate_ok = if let Verdict::Ok(upstream) = verdict {
        ensure_gate(app, upstream)
    } else {
        lock(&shared(app).gate).take().inspect(|g| g.close());
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
