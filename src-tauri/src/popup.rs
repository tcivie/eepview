// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The toolbar popups: one at a time, in the `popup` webview. Pure state; the shell places
//! and shows the webview (`docs/wiki/browser-shell.md`, "Toolbar popups").

use serde::{Deserialize, Serialize};

use crate::layout::{self, Align, Rect, Size};

/// The kind of a toolbar popup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// The address suggestions.
    Suggestions,
    /// The main menu.
    Menu,
    /// The router panel.
    Router,
    /// The router hint (hover on the router dot).
    Hint,
}

impl Kind {
    /// The anchor edge the popup lines up with: the left edge for the suggestions, the
    /// right edge for the buttons at the right end of the toolbar.
    #[must_use]
    pub fn align(self) -> Align {
        match self {
            Self::Suggestions => Align::Start,
            Self::Menu | Self::Router | Self::Hint => Align::End,
        }
    }

    /// True when the popup takes the keyboard focus (the menu and the router panel).
    #[must_use]
    pub fn takes_focus(self) -> bool {
        matches!(self, Self::Menu | Self::Router)
    }
}

/// A popup that closed, or the one that is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Closed {
    /// Its id.
    pub id: u64,
    /// Its kind.
    pub kind: Kind,
}

#[derive(Debug, Clone, Copy)]
struct Open {
    id: u64,
    kind: Kind,
    anchor: Rect,
}

/// The one-popup state.
#[derive(Debug, Default)]
pub struct Popups {
    open: Option<Open>,
    last_id: u64,
}

impl Popups {
    /// Opens a popup of `kind` under `anchor` and returns its new id. It returns the popup it
    /// closed when one of another kind was open; the same kind is updated in place.
    pub fn open(&mut self, kind: Kind, anchor: Rect) -> (u64, Option<Closed>) {
        self.last_id += 1;
        let id = self.last_id;
        let replaced = self.open.filter(|o| o.kind != kind).map(Self::closed);
        self.open = Some(Open { id, kind, anchor });
        (id, replaced)
    }

    /// Where the open popup `id` goes for a card of natural `size` in a window of `window`.
    /// `None` for a stale id or a size that is not finite and positive.
    pub fn size(&mut self, id: u64, size: Size, window: Size) -> Option<Rect> {
        let open = self.open.filter(|o| o.id == id)?;
        let valid = |v: f64| v.is_finite() && v > 0.0;
        if !(valid(size.0) && valid(size.1)) {
            return None;
        }
        Some(layout::popup(open.anchor, size, window, open.kind.align()))
    }

    /// Closes the open popup when `id` is it, or is `None`. A stale id does nothing.
    pub fn close(&mut self, id: Option<u64>) -> Option<Closed> {
        let open = self.open?;
        if id.is_some_and(|id| id != open.id) {
            return None;
        }
        self.open = None;
        Some(Self::closed(open))
    }

    /// The open popup.
    #[must_use]
    pub fn current(&self) -> Option<Closed> {
        self.open.map(Self::closed)
    }

    fn closed(open: Open) -> Closed {
        Closed {
            id: open.id,
            kind: open.kind,
        }
    }
}
