//! The wire types of the IPC contract (`docs/ipc.md`).

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
}

/// The filter of `history_query`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct HistoryQuery {
    /// Text in the URL or the title.
    #[serde(default)]
    pub q: Option<String>,
    /// Only visits before this time, Unix ms.
    #[serde(default)]
    pub before: Option<u64>,
    /// Most entries to return.
    #[serde(default)]
    pub limit: Option<usize>,
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
    /// `verifying`, `ok`, `building`, `down`, `not-i2p` or `outproxy`.
    pub state: &'static str,
    /// `host:port` of the HTTP proxy.
    pub proxy: String,
    /// Router version, when known.
    pub version: Option<String>,
    /// A short explanation for the user.
    pub detail: Option<String>,
}

impl RouterStatus {
    /// True when web tabs may exist.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.state == "ok"
    }
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
        let status = RouterStatus {
            state: "ok",
            proxy: String::new(),
            version: None,
            detail: None,
        };
        assert!(status.is_ok());
    }
}
