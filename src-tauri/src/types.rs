// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The wire types of the IPC contract (`docs/wiki/ipc-contract.md`).

use serde::{Deserialize, Serialize};

/// One tab, as the toolbar sees it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabInfo {
    /// Tab id.
    pub id: u32,
    /// Address-bar text.
    pub url: String,
    /// Page title.
    pub title: String,
    /// `"internal"` or `"web"`.
    pub kind: &'static str,
    /// Load and history state.
    #[serde(flatten)]
    pub nav: NavFlags,
    /// Zoom factor, 1.0 = 100 %.
    pub zoom: f64,
    /// Selection, JavaScript and bookmark state.
    #[serde(flatten)]
    pub marks: TabMarks,
    /// The 32 px site icon as a `data:image/png;base64,` URL; `None` for none.
    pub icon: Option<String>,
}

/// The load and history flags of a [`TabInfo`] (flattened on the wire).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavFlags {
    /// A load is in progress.
    pub loading: bool,
    /// Back is possible.
    pub can_back: bool,
    /// Forward is possible.
    pub can_forward: bool,
}

/// The other flags of a [`TabInfo`] (flattened on the wire).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabMarks {
    /// The tab is the active one.
    pub active: bool,
    /// Page JavaScript is on for this site.
    pub js_on: bool,
    /// The URL is bookmarked.
    pub bookmarked: bool,
}

/// The answer of `navigate`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NavResult {
    /// The input was accepted.
    pub ok: bool,
    /// Why not: `not-i2p`, `router-down` or `invalid`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<&'static str>,
}

impl NavResult {
    /// Accepted.
    #[must_use]
    pub fn ok() -> Self {
        Self {
            ok: true,
            reason: None,
        }
    }

    /// Refused for `reason`.
    #[must_use]
    pub fn refused(reason: &'static str) -> Self {
        Self {
            ok: false,
            reason: Some(reason),
        }
    }
}

/// A saved bookmark.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bookmark {
    /// Id.
    pub id: String,
    /// Address.
    pub url: String,
    /// Title.
    pub title: String,
    /// Folder name, one level deep.
    #[serde(default)]
    pub folder: Option<String>,
    /// Creation time, Unix ms.
    #[serde(default)]
    pub created: u64,
    /// The 64 px site icon as a `data:image/png;base64,` URL. Filled only on the way to the
    /// UI: the store never keeps it, and input never sets it.
    #[serde(default, skip_deserializing)]
    pub icon: Option<String>,
}

/// The fields of a new bookmark.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NewBookmark {
    /// Address.
    pub url: String,
    /// Title.
    #[serde(default)]
    pub title: String,
    /// Folder name.
    #[serde(default)]
    pub folder: Option<String>,
}

/// One visited URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Id.
    pub id: String,
    /// Address.
    pub url: String,
    /// Last title.
    pub title: String,
    /// Last visit, Unix ms.
    pub visited: u64,
    /// Visit count.
    pub visits: u32,
    /// The 32 px site icon as a `data:image/png;base64,` URL. Filled only on the way to the
    /// UI: the store never keeps it, and input never sets it.
    #[serde(default, skip_deserializing)]
    pub icon: Option<String>,
}

/// The filter of `history_query`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct HistoryQuery {
    /// Text in the URL or the title.
    #[serde(default)]
    pub q: Option<String>,
    /// Page cursor: only entries after this one in (visited desc, id desc) order.
    #[serde(default)]
    pub before: Option<Cursor>,
    /// Most entries to return.
    #[serde(default)]
    pub limit: Option<usize>,
}

/// A history page cursor: the last entry of the previous page.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Cursor {
    /// `{visited, id}` (contract v1.1).
    Entry {
        /// Visit time of the last entry, Unix ms.
        visited: u64,
        /// Id of the last entry.
        id: String,
    },
    /// A bare time, Unix ms (contract v1).
    Time(u64),
}

impl Cursor {
    /// True when `(visited, id)` sorts after the cursor (older, or same time and smaller id).
    #[must_use]
    pub fn admits(&self, visited: u64, id: &str) -> bool {
        match self {
            Self::Entry {
                visited: v,
                id: last,
            } => (visited, id) < (*v, last.as_str()),
            Self::Time(t) => visited < *t,
        }
    }
}

/// One address-bar suggestion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Suggestion {
    /// Address.
    pub url: String,
    /// Title.
    pub title: String,
    /// `"bookmark"` or `"history"`.
    pub source: &'static str,
}

/// What the browser knows about the router.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RouterStatus {
    /// `verifying`, `ok`, `building`, `down` or `not-i2p`.
    pub state: &'static str,
    /// `host:port` of the HTTP proxy.
    pub proxy: String,
    /// Router version, when known.
    pub version: Option<String>,
    /// A short explanation for the user.
    pub detail: Option<String>,
    /// The user paused the connection: the gatekeeper is closed until resume.
    pub paused: bool,
    /// eepview runs the router itself (false for an external router).
    pub managed: bool,
}

impl RouterStatus {
    /// True when web tabs may exist: verified and not paused.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.state == "ok" && !self.paused
    }
}

/// The `router_control` and `console_open` answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ControlResult {
    /// True when the router did what was asked.
    pub ok: bool,
    /// Why not: `external` while eepview does not run the router; `no-console` or `no-page`
    /// for `console_open`.
    pub reason: Option<&'static str>,
}

/// The `find-result` event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FindResult {
    /// The text searched.
    pub query: String,
    /// Match count, when the engine reports one.
    pub matches: Option<u32>,
    /// The 1-based active match, when known.
    pub active: Option<u32>,
}

/// `chrome_insets()` and the `chrome-insets-changed` event: the space the tab strip leaves
/// on the left for the window buttons.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ChromeInsets {
    /// Points from the left window edge.
    pub left: f64,
}

/// The `toast` event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Toast {
    /// `info` or `warn`.
    pub kind: &'static str,
    /// Message.
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nav_result_shape() {
        let ok = serde_json::to_string(&NavResult::ok()).unwrap();
        assert_eq!(ok, r#"{"ok":true}"#);
        let no = serde_json::to_string(&NavResult::refused("not-i2p")).unwrap();
        assert_eq!(no, r#"{"ok":false,"reason":"not-i2p"}"#);
    }

    #[test]
    fn tab_info_is_camel_case() {
        let info = TabInfo {
            id: 1,
            url: "eepview://home".into(),
            title: String::new(),
            kind: "internal",
            nav: NavFlags {
                can_forward: true,
                ..NavFlags::default()
            },
            zoom: 1.0,
            marks: TabMarks::default(),
            icon: None,
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["canForward"], true);
        assert_eq!(json["jsOn"], false);
        assert_eq!(json["loading"], false);
        assert!(json.get("nav").is_none());
    }

    #[test]
    fn inputs_take_defaults() {
        let b: NewBookmark = serde_json::from_str(r#"{"url":"http://a.i2p/"}"#).unwrap();
        assert_eq!(b.folder, None);
        let q: HistoryQuery = serde_json::from_str("{}").unwrap();
        assert_eq!(q, HistoryQuery::default());
        let c: HistoryQuery =
            serde_json::from_str(r#"{"before":{"visited":5,"id":"b"},"limit":2}"#).unwrap();
        let cursor = c.before.unwrap();
        assert!(cursor.admits(4, "z") && cursor.admits(5, "a"));
        assert!(!cursor.admits(5, "b") && !cursor.admits(6, "a"));
        let t: HistoryQuery = serde_json::from_str(r#"{"before":5}"#).unwrap();
        assert!(t.before.unwrap().admits(4, "x"));
        let status = RouterStatus {
            state: "ok",
            proxy: String::new(),
            version: None,
            detail: None,
            paused: false,
            managed: false,
        };
        assert!(status.is_ok());
        let paused = RouterStatus {
            paused: true,
            ..status
        };
        assert!(!paused.is_ok());
    }
}
