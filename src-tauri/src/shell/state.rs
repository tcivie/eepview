//! The state the shell shares between commands, engine callbacks and the router watcher.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::menu::MenuItem;
use tauri::{Manager, Runtime, Url};

use crate::core::Core;
use crate::net::gatekeeper::Gatekeeper;

/// Shared shell state, managed by Tauri.
pub struct Shared<R: Runtime> {
    /// The browser core.
    pub core: Mutex<Core>,
    /// The running gatekeeper; `None` while the router is not verified.
    pub gate: Mutex<Option<Arc<Gatekeeper>>>,
    /// The label of the live webview of each tab.
    pub labels: Mutex<HashMap<u32, String>>,
    /// The origin of the bundled pages (`tauri://localhost` or the dev server).
    pub base: Mutex<Option<Url>>,
    /// The Stop menu item, enabled only while the active tab loads.
    pub stop_item: Mutex<Option<MenuItem<R>>>,
    generation: AtomicU64,
}

impl<R: Runtime> Shared<R> {
    /// New shared state around a core.
    #[must_use]
    pub fn new(core: Core) -> Self {
        Self {
            core: Mutex::new(core),
            gate: Mutex::new(None),
            labels: Mutex::new(HashMap::new()),
            base: Mutex::new(None),
            stop_item: Mutex::new(None),
            generation: AtomicU64::new(0),
        }
    }

    /// A fresh `tab-<id>-<n>` label: a rebuilt webview never reuses a closing one's label.
    #[must_use]
    pub fn next_label(&self, tab: u32) -> String {
        let n = self.generation.fetch_add(1, Ordering::SeqCst);
        format!("tab-{tab}-{n}")
    }
}

/// Locks a mutex, taking the data even when another thread panicked while holding it.
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The shared state of an app.
pub fn shared<R: Runtime, M: Manager<R>>(app: &M) -> tauri::State<'_, Shared<R>> {
    app.state::<Shared<R>>()
}

/// Unix time in ms.
#[must_use]
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}
