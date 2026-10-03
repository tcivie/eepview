// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The browser state machine. Pure: commands and engine events go in, [`Effect`]s come out,
//! and the shell (`shell/`) carries them out. No Tauri runtime here, so all of it is tested.

pub mod find;
mod keys;
mod library;
mod navigation;
mod page;
pub mod router;
mod site_icons;
mod tab_ops;

#[cfg(test)]
mod tests;

use crate::net::stats::{History as StatsHistory, RouterStats, Sample};
use std::path::PathBuf;
use tauri::Url;

use crate::hover::{Debounce, HoverText};
use crate::icons::IconStore;
use crate::nav::{host_of, internal_with, is_web};
use crate::store::bookmarks::Bookmarks;
use crate::store::history::History;
use crate::store::settings::Settings;
use crate::store::site_prefs::SitePrefs;
use crate::tabs::{Tab, Tabs};
use crate::types::{FindResult, NavFlags, RouterStatus, TabInfo, TabMarks, Toast};

/// Where the stores live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    /// `bookmarks.json` (config dir).
    pub bookmarks: PathBuf,
    /// `history.json` (data dir).
    pub history: PathBuf,
    /// `settings.json` (config dir).
    pub settings: PathBuf,
    /// `sites.json`: per-site zoom and JS (config dir).
    pub sites: PathBuf,
    /// `icons/`: the site icons (data dir).
    pub icons: PathBuf,
}

/// Something the shell must do.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Send an event to the `toolbar` and `internal` webviews.
    Emit(Event),
    /// Act on a `tab-*` webview.
    Web(WebOp),
    /// Give keyboard focus to the toolbar.
    FocusToolbar,
    /// Give keyboard focus to the content area (the web tab or the internal page shown).
    FocusContent,
    /// Lay the webviews out again and show the right one.
    Layout,
    /// Call [`Core::hover_expire`] with this generation after the show delay.
    HoverLater(u64),
    /// Ask this host for its icon through the gatekeeper, then call [`Core::icon_fetched`].
    FetchIcon(String),
}

/// An event of the contract.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// `tabs-changed`.
    TabsChanged,
    /// `tab-updated` for one tab.
    TabUpdated(u32),
    /// `find-result`.
    Find(FindResult),
    /// `router-status`.
    Router,
    /// `bookmarks-changed`.
    Bookmarks,
    /// `history-changed`.
    History,
    /// `settings-changed`.
    Settings,
    /// `shortcut` for an action the UI handles.
    Shortcut(&'static str),
    /// `toast`.
    Toast(Toast),
    /// `link-hover`: the status bubble text, or `None` to hide it.
    Hover(Option<HoverText>),
    /// `icons-changed`.
    Icons,
}

/// An operation on a `tab-*` webview.
#[derive(Debug, Clone, PartialEq)]
pub enum WebOp {
    /// Load `url`. Build the webview first when there is none, or when `rebuild` is set (its
    /// JS flag must change).
    Load(Load),
    /// Close the webview.
    Destroy(u32),
    /// An engine call on the live webview.
    Engine(u32, EngineOp),
}

/// The parameters of [`WebOp::Load`].
#[derive(Debug, Clone, PartialEq)]
pub struct Load {
    /// Tab id.
    pub tab: u32,
    /// An allowed I2P URL.
    pub url: String,
    /// Page JavaScript on.
    pub js: bool,
    /// Throw the old webview away first.
    pub rebuild: bool,
    /// Incognito (no cookies or cache after exit).
    pub private: bool,
    /// Page zoom.
    pub zoom: f64,
}

/// An engine call.
#[derive(Debug, Clone, PartialEq)]
pub enum EngineOp {
    /// One step back in the engine list.
    Back,
    /// One step forward in the engine list.
    Forward,
    /// Reload.
    Reload,
    /// Reload, bypassing the cache.
    HardReload,
    /// Stop loading.
    Stop,
    /// Set the page zoom.
    Zoom(f64),
    /// Find text.
    Find(FindOp),
    /// Clear find highlights.
    FindClear,
}

/// The parameters of [`EngineOp::Find`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindOp {
    /// Text.
    pub query: String,
    /// Search forward.
    pub forward: bool,
    /// Match case.
    pub match_case: bool,
    /// A new search (count the matches) rather than next/previous.
    pub fresh: bool,
}

/// Which webview the content area shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    /// The `internal` webview with this `eepview://` page.
    Internal(String),
    /// The `tab-<id>` webview.
    Web(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FindState {
    tab: u32,
    query: String,
    match_case: bool,
    matches: Option<u32>,
    active: Option<u32>,
}

/// The whole browser state.
#[derive(Debug)]
pub struct Core {
    tabs: Tabs,
    bookmarks: Bookmarks,
    history: History,
    settings: Settings,
    prefs: SitePrefs,
    icons: IconStore,
    icon_jobs: site_icons::IconJobs,
    paths: Option<Paths>,
    router: RouterStatus,
    find: Option<FindState>,
    find_open: bool,
    toolbar_request: f64,
    hover: Debounce,
    js_forced_off: bool,
    stats: StatsHistory,
    /// The link under the mouse in the active tab: a refused navigation to it is a click.
    pointed: Option<Url>,
    /// The tab whose next commit takes keyboard focus: its navigation came from the address bar.
    focus_on_commit: Option<u32>,
}

impl Core {
    /// A core with the stores read from `paths` (or fresh ones for `None`), one home tab,
    /// and the router not verified yet.
    #[must_use]
    pub fn new(paths: Option<Paths>, proxy: &str, now: u64) -> Self {
        let mut core = Self {
            tabs: Tabs::new(),
            bookmarks: paths.as_ref().map_or_else(
                || Bookmarks::seeded(now),
                |p| Bookmarks::load(&p.bookmarks, now),
            ),
            history: paths
                .as_ref()
                .map_or_else(History::default, |p| History::load(&p.history)),
            settings: paths
                .as_ref()
                .map_or_else(Settings::default, |p| Settings::load(&p.settings)),
            prefs: paths
                .as_ref()
                .map_or_else(SitePrefs::default, |p| SitePrefs::load(&p.sites)),
            icons: paths
                .as_ref()
                .map_or_else(IconStore::default, |p| IconStore::load(&p.icons)),
            icon_jobs: site_icons::IconJobs::default(),
            paths,
            router: verifying(proxy),
            find: None,
            find_open: false,
            toolbar_request: 0.0,
            hover: Debounce::default(),
            js_forced_off: false,
            stats: StatsHistory::default(),
            pointed: None,
            focus_on_commit: None,
        };
        // Effects are dropped: no UI listens yet, and a save error shows on the next save.
        let _ = core.sweep_icons();
        let home = core.home_url();
        core.tabs.open(&home, crate::tabs::Place::End, true);
        core
    }

    /// The configured homepage.
    #[must_use]
    pub fn home_url(&self) -> String {
        self.settings.homepage.clone()
    }

    /// The router status.
    #[must_use]
    pub fn router(&self) -> &RouterStatus {
        &self.router
    }

    /// The settings.
    #[must_use]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// The tab strip.
    #[must_use]
    pub fn tabs(&self) -> &Tabs {
        &self.tabs
    }

    /// True while the find bar is open.
    #[must_use]
    pub fn find_open(&self) -> bool {
        self.find_open
    }

    /// The toolbar height the UI asked for (0 for the default).
    #[must_use]
    pub fn toolbar_request(&self) -> f64 {
        self.toolbar_request
    }

    /// Sets the toolbar height request (0 for the default).
    pub fn set_toolbar_request(&mut self, px: f64) -> Vec<Effect> {
        self.toolbar_request = if px.is_finite() { px.max(0.0) } else { 0.0 };
        vec![Effect::Layout]
    }

    /// The webview the content area shows for the active tab.
    #[must_use]
    pub fn view(&self) -> View {
        let Some(tab) = self.tabs.active() else {
            return View::Internal("eepview://home".into());
        };
        if !is_web(&tab.url) {
            return View::Internal(tab.url.clone());
        }
        if self.router.is_ok() && tab.web_js.is_some() {
            return View::Web(tab.id);
        }
        let mut params = vec![("url", tab.url.as_str()), ("state", self.router.state)];
        if self.router.paused {
            params.push(("reason", "paused"));
        }
        View::Internal(internal_with("router-down", &params))
    }

    /// Every tab, in strip order.
    #[must_use]
    pub fn tab_infos(&self) -> Vec<TabInfo> {
        self.tabs.iter().map(|t| self.info_of(t)).collect()
    }

    /// One tab.
    #[must_use]
    pub fn tab_info(&self, id: u32) -> Option<TabInfo> {
        self.tabs.get(id).map(|t| self.info_of(t))
    }

    fn info_of(&self, tab: &Tab) -> TabInfo {
        let web = is_web(&tab.url);
        let host = host_of(&tab.url);
        TabInfo {
            id: tab.id,
            url: tab.url.clone(),
            title: tab.title.clone(),
            kind: if web { "web" } else { "internal" },
            nav: NavFlags {
                loading: tab.loading,
                can_back: tab.session.can_back(),
                can_forward: tab.session.can_forward(),
            },
            zoom: host.as_deref().map_or(1.0, |h| self.zoom_of(h)),
            marks: TabMarks {
                active: tab.id == self.tabs.active_id(),
                js_on: host
                    .as_deref()
                    .map_or(self.settings.js_default, |h| self.js_of(h)),
                bookmarked: self.bookmarks.find(&tab.url).is_some(),
            },
            icon: if web { self.small_icon(&tab.url) } else { None },
        }
    }

    fn zoom_of(&self, host: &str) -> f64 {
        self.prefs.zoom(host, self.settings.zoom_default)
    }

    fn js_of(&self, host: &str) -> bool {
        !self.js_forced_off && self.prefs.js(host, self.settings.js_default)
    }

    /// Records one router stats sample (every watcher tick).
    pub fn record_stats(&mut self, now: u64, stats: &RouterStats) {
        self.stats.record(now, stats);
    }

    /// Records a console stats sample, unless the newest one is less than 4 s old.
    pub fn record_stats_spaced(&mut self, now: u64, stats: &RouterStats) {
        self.stats.record_spaced(now, stats);
    }

    /// The router bandwidth of the last 10 minutes.
    #[must_use]
    pub fn stats_history(&self) -> Vec<Sample> {
        self.stats.samples()
    }

    /// Turns page JavaScript off for every site for this run (`EEPVIEW_JS=off`).
    pub fn force_js_off(&mut self) {
        self.js_forced_off = true;
    }

    fn toast(kind: &'static str, text: impl Into<String>) -> Effect {
        Effect::Emit(Event::Toast(Toast {
            kind,
            text: text.into(),
        }))
    }
}

/// The router status before the first VERIFY.
fn verifying(proxy: &str) -> RouterStatus {
    RouterStatus {
        state: "verifying",
        proxy: proxy.to_owned(),
        version: None,
        detail: None,
        paused: false,
        managed: false,
    }
}

/// The title of an internal page: its name with a capital letter.
#[must_use]
pub fn internal_title(url: &str) -> String {
    let name = url
        .strip_prefix("eepview://")
        .unwrap_or(url)
        .split(['?', '/'])
        .next()
        .unwrap_or("");
    let mut chars = name.chars();
    chars.next().map_or_else(String::new, |first| {
        first
            .to_uppercase()
            .chain(chars)
            .collect::<String>()
            .replace('-', " ")
    })
}
