// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The state the shell shares between commands, engine callbacks and the router watcher.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::menu::MenuItem;
use tauri::{Manager, Runtime, Url};

use crate::context_menu::Target;
use crate::core::Core;
use crate::net::console::VerifiedConsole;
use crate::net::gatekeeper::Gatekeeper;
use crate::popup::Popups;

/// Shared shell state, managed by Tauri.
pub struct Shared<R: Runtime> {
    /// The browser core.
    pub core: Mutex<Core>,
    /// The running gatekeeper; `None` while the router is not verified.
    pub gate: Mutex<Option<Arc<Gatekeeper>>>,
    /// The detected router console; `None` until a detection finds one.
    pub console: Mutex<Option<VerifiedConsole>>,
    /// The console loop epoch: `console::stop` moves it on, and a loop of an older epoch
    /// ends at its next tick.
    pub console_epoch: AtomicU64,
    /// The epoch of the running console re-check loop, or 0.
    pub console_watch: AtomicU64,
    /// The epoch of the running console retry loop, or 0.
    pub console_retry: AtomicU64,
    /// True once the console rule list is attached to the console view.
    pub console_armed: AtomicBool,
    /// The label of the live webview of each tab.
    pub labels: Mutex<HashMap<u32, String>>,
    /// The origin of the bundled pages (`tauri://localhost` or the dev server).
    pub base: Mutex<Option<Url>>,
    /// The Stop menu item, enabled only while the active tab loads.
    pub stop_item: Mutex<Option<MenuItem<R>>>,
    /// The last full-screen state sent to the UI.
    pub fullscreen: AtomicBool,
    /// The window-button frames last measured (left edge, right edge), macOS only.
    pub buttons: Mutex<Option<(f64, f64)>>,
    /// The toolbar popup that is open, if any.
    pub popups: Mutex<Popups>,
    /// The label of the webview of the last context menu, and its target: the chosen item
    /// acts on them.
    pub menu_target: Mutex<Option<(String, Target)>>,
    generation: AtomicU64,
}

impl<R: Runtime> Shared<R> {
    /// New shared state around a core.
    #[must_use]
    pub fn new(core: Core) -> Self {
        Self {
            core: Mutex::new(core),
            gate: Mutex::new(None),
            console: Mutex::new(None),
            console_epoch: AtomicU64::new(1),
            console_watch: AtomicU64::new(0),
            console_retry: AtomicU64::new(0),
            console_armed: AtomicBool::new(false),
            labels: Mutex::new(HashMap::new()),
            base: Mutex::new(None),
            stop_item: Mutex::new(None),
            fullscreen: AtomicBool::new(false),
            buttons: Mutex::new(None),
            popups: Mutex::new(Popups::default()),
            menu_target: Mutex::new(None),
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

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;
    use crate::shell::testing::{Mock, bare};

    #[test]
    fn labels_never_repeat() {
        let state = Shared::<Mock>::new(Core::new(None, "127.0.0.1:4444", 0));
        assert_eq!(state.next_label(3), "tab-3-0");
        assert_eq!(state.next_label(3), "tab-3-1");
        assert_eq!(state.next_label(4), "tab-4-2");
    }

    #[test]
    fn lock_takes_the_data_of_a_poisoned_mutex() {
        let m = Arc::new(Mutex::new(5));
        let other = Arc::clone(&m);
        let _ = thread::spawn(move || {
            let _guard = other.lock().unwrap();
            panic!("poison the mutex");
        })
        .join();
        assert!(m.is_poisoned());
        assert_eq!(*lock(&m), 5);
    }

    #[test]
    fn now_is_unix_ms() {
        assert!(now_ms() > 1_700_000_000_000);
    }

    #[test]
    fn shared_reads_the_managed_state() {
        let app = bare();
        assert!(lock(&shared(app.handle()).gate).is_none());
        assert!(lock(&shared(app.handle()).labels).is_empty());
    }
}
