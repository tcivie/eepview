// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Bookmarks, history, settings and suggestions, with their files.

use std::io;

use serde_json::Value;

use super::{Core, Effect, Event};
use crate::store::history::range_ms;
use crate::store::settings::Settings;
use crate::suggest::suggest;
use crate::types::{Bookmark, HistoryEntry, HistoryQuery, NewBookmark, Suggestion};

impl Core {
    pub(super) fn saved(result: io::Result<()>, what: &str) -> Vec<Effect> {
        match result {
            Ok(()) => Vec::new(),
            Err(e) => vec![Self::toast("warn", format!("Could not save {what}: {e}"))],
        }
    }

    fn save_bookmarks(&self) -> Vec<Effect> {
        let Some(p) = &self.paths else {
            return Vec::new();
        };
        Self::saved(self.bookmarks.save(&p.bookmarks), "bookmarks")
    }

    pub(super) fn save_history(&self) -> Vec<Effect> {
        let Some(p) = &self.paths else {
            return Vec::new();
        };
        Self::saved(self.history.save(&p.history), "history")
    }

    fn save_settings(&self) -> Vec<Effect> {
        let Some(p) = &self.paths else {
            return Vec::new();
        };
        Self::saved(self.settings.save(&p.settings), "settings")
    }

    pub(super) fn save_prefs(&self) -> Vec<Effect> {
        let Some(p) = &self.paths else {
            return Vec::new();
        };
        Self::saved(self.prefs.save(&p.sites), "site settings")
    }

    /// Effects after the bookmark list changed: save, tell the UI, refresh the star.
    fn bookmarks_changed(&self) -> Vec<Effect> {
        let mut fx = self.save_bookmarks();
        fx.push(Effect::Emit(Event::Bookmarks));
        fx.push(Effect::Emit(Event::TabsChanged));
        fx
    }

    /// `bookmarks_list()`.
    #[must_use]
    pub fn bookmarks_list(&self) -> Vec<Bookmark> {
        let list = self.bookmarks.list().iter().cloned();
        list.map(|b| self.with_icon(b)).collect()
    }

    /// `bookmark_add(bookmark)`.
    ///
    /// # Errors
    ///
    /// Fails for a URL that is not an I2P site or an internal page.
    pub fn bookmark_add(
        &mut self,
        new: &NewBookmark,
        now: u64,
    ) -> Result<(Bookmark, Vec<Effect>), String> {
        let bookmark = self.bookmarks.add(new, now)?;
        Ok((self.with_icon(bookmark), self.bookmarks_changed()))
    }

    /// `bookmark_update(bookmark)`.
    pub fn bookmark_update(&mut self, bookmark: &Bookmark) -> Vec<Effect> {
        if self.bookmarks.update(bookmark) {
            let mut fx = self.bookmarks_changed();
            fx.extend(self.sweep_icons());
            fx
        } else {
            Vec::new()
        }
    }

    /// `bookmark_remove(id)`.
    pub fn bookmark_remove(&mut self, id: &str) -> Vec<Effect> {
        if self.bookmarks.remove(id) {
            let mut fx = self.bookmarks_changed();
            fx.extend(self.sweep_icons());
            fx
        } else {
            Vec::new()
        }
    }

    /// `bookmark_find(url)`.
    #[must_use]
    pub fn bookmark_find(&self, url: &str) -> Option<Bookmark> {
        self.bookmarks.find(url).cloned().map(|b| self.with_icon(b))
    }

    /// `bookmarks_export()`.
    #[must_use]
    pub fn bookmarks_export(&self) -> String {
        self.bookmarks.export()
    }

    /// `bookmarks_import(json)`.
    ///
    /// # Errors
    ///
    /// Fails when the text is not bookmark JSON.
    pub fn bookmarks_import(
        &mut self,
        json: &str,
        now: u64,
    ) -> Result<(usize, Vec<Effect>), String> {
        let added = self.bookmarks.import(json, now)?;
        let fx = if added > 0 {
            self.bookmarks_changed()
        } else {
            Vec::new()
        };
        Ok((added, fx))
    }

    /// Bookmarks the active page, or removes its bookmark (Cmd+D).
    pub fn bookmark_toggle(&mut self, now: u64) -> Vec<Effect> {
        let Some(tab) = self.tabs.active() else {
            return Vec::new();
        };
        let (url, title) = (tab.url.clone(), tab.title.clone());
        if let Some(id) = self.bookmarks.find(&url).map(|b| b.id.clone()) {
            return self.bookmark_remove(&id);
        }
        let new = NewBookmark {
            url,
            title,
            folder: None,
        };
        self.bookmark_add(&new, now)
            .map(|(_, fx)| fx)
            .unwrap_or_default()
    }

    /// `history_query(query)`.
    #[must_use]
    pub fn history_query(&self, query: &HistoryQuery) -> Vec<HistoryEntry> {
        self.with_icons(self.history.query(query))
    }

    /// `history_remove(id)`.
    pub fn history_remove(&mut self, id: &str) -> Vec<Effect> {
        if !self.history.remove(id) {
            return Vec::new();
        }
        let mut fx = self.save_history();
        fx.push(Effect::Emit(Event::History));
        fx.extend(self.sweep_icons());
        fx
    }

    /// `history_clear(range)`.
    ///
    /// # Errors
    ///
    /// Fails for an unknown range name.
    pub fn history_clear(&mut self, range: &str, now: u64) -> Result<Vec<Effect>, String> {
        self.history.clear(range_ms(range)?, now);
        let mut fx = self.save_history();
        fx.push(Effect::Emit(Event::History));
        fx.extend(self.sweep_icons());
        Ok(fx)
    }

    /// `suggest(input)`.
    #[must_use]
    pub fn suggest(&self, input: &str, now: u64) -> Vec<Suggestion> {
        suggest(input, self.bookmarks.list(), self.history.entries(), now)
    }

    /// `settings_set(patch)`: applies, saves, and rebuilds tabs whose JS flag changed.
    ///
    /// # Errors
    ///
    /// Fails for a bad patch; nothing changes then.
    pub fn settings_set(&mut self, patch: &Value) -> Result<(Settings, Vec<Effect>), String> {
        self.settings = self.settings.patched(patch)?;
        let mut fx = self.save_settings();
        fx.push(Effect::Emit(Event::Settings));
        fx.extend(self.apply_js());
        Ok((self.settings.clone(), fx))
    }
}
