// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! User settings, patched field by field from the UI.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{VERSION, read_json, write_json};
use crate::store::bookmarks::normalise;

/// Smallest and largest page zoom.
pub const ZOOM_RANGE: (f64, f64) = (0.3, 3.0);

/// Colour theme.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Follow the OS.
    #[default]
    System,
    /// Light.
    Light,
    /// Dark.
    Dark,
}

/// History settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistorySettings {
    /// Record visits.
    pub enabled: bool,
}

impl Default for HistorySettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// All settings (the contract `Settings` type).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// `eepview://home` or an I2P URL.
    pub homepage: String,
    /// Colour theme.
    pub theme: Theme,
    /// Page JavaScript for sites with no per-site choice. On by default (owner decision):
    /// the network layers below JS hold with JS on.
    pub js_default: bool,
    /// History settings.
    pub history: HistorySettings,
    /// Keep cookies and cache after exit.
    pub keep_cookies: bool,
    /// Zoom for sites with no per-site zoom.
    pub zoom_default: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            homepage: "eepview://home".into(),
            theme: Theme::System,
            js_default: true,
            history: HistorySettings::default(),
            keep_cookies: false,
            zoom_default: 1.0,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct File {
    version: u32,
    settings: Settings,
}

impl Settings {
    /// Reads the file, or the defaults.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        read_json::<File>(path).map_or_else(Self::default, |f| f.settings)
    }

    /// Writes the file.
    ///
    /// # Errors
    ///
    /// Fails when the file cannot be written.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        write_json(
            path,
            &File {
                version: VERSION,
                settings: self.clone(),
            },
        )
    }

    /// These settings with `patch` (a partial `Settings` object) applied and checked.
    ///
    /// # Errors
    ///
    /// Fails when the patch is not an object, a field has the wrong type, or the homepage is
    /// not an I2P site or an internal page.
    pub fn patched(&self, patch: &Value) -> Result<Self, String> {
        let mut merged = serde_json::to_value(self).map_err(|e| e.to_string())?;
        merge(&mut merged, patch)?;
        let mut next: Self = serde_json::from_value(merged).map_err(|e| e.to_string())?;
        next.homepage = normalise(&next.homepage)
            .ok_or_else(|| format!("homepage is not an I2P address: {}", next.homepage))?;
        next.zoom_default = clamp_zoom(next.zoom_default);
        Ok(next)
    }
}

/// Keeps a zoom factor inside [`ZOOM_RANGE`]; a non-number becomes 1.0.
#[must_use]
pub fn clamp_zoom(zoom: f64) -> f64 {
    if zoom.is_finite() {
        zoom.clamp(ZOOM_RANGE.0, ZOOM_RANGE.1)
    } else {
        1.0
    }
}

/// Merges the keys of `patch` into `base`, one level into nested objects.
fn merge(base: &mut Value, patch: &Value) -> Result<(), String> {
    let (Some(base), Some(patch)) = (base.as_object_mut(), patch.as_object()) else {
        return Err("settings patch must be an object".into());
    };
    for (key, value) in patch {
        match (base.get_mut(key), value) {
            (Some(Value::Object(inner)), Value::Object(part)) => {
                inner.extend(part.iter().map(|(k, v)| (k.clone(), v.clone())));
            }
            (Some(slot), _) => *slot = value.clone(),
            (None, _) => return Err(format!("unknown setting: {key}")),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::testdir;
    use serde_json::json;

    #[test]
    fn defaults_have_js_on() {
        let s = Settings::default();
        assert!(s.js_default);
        assert!(s.history.enabled);
        assert!(!s.keep_cookies);
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["jsDefault"], true);
        assert_eq!(json["theme"], "system");
        assert_eq!(json["history"]["enabled"], true);
    }

    #[test]
    fn patch_fields() {
        let s = Settings::default();
        let next = s
            .patched(&json!({"theme": "dark", "history": {"enabled": false}, "jsDefault": false}))
            .unwrap();
        assert_eq!(next.theme, Theme::Dark);
        assert!(!next.history.enabled);
        assert!(!next.js_default);
        let home = s.patched(&json!({"homepage": "Stats.I2P"})).unwrap();
        assert_eq!(home.homepage, "http://stats.i2p/");
        let zoom = s.patched(&json!({"zoomDefault": 9.0})).unwrap();
        assert!((zoom.zoom_default - ZOOM_RANGE.1).abs() < f64::EPSILON);
    }

    #[test]
    fn bad_patches() {
        let s = Settings::default();
        assert!(
            s.patched(&json!({"homepage": "http://example.com/"}))
                .is_err()
        );
        assert!(s.patched(&json!({"nope": 1})).is_err());
        assert!(s.patched(&json!({"theme": "pink"})).is_err());
        assert!(s.patched(&json!([1])).is_err());
    }

    #[test]
    fn zoom_clamp() {
        assert!((clamp_zoom(f64::NAN) - 1.0).abs() < f64::EPSILON);
        assert!((clamp_zoom(0.1) - ZOOM_RANGE.0).abs() < f64::EPSILON);
    }

    #[test]
    fn load_and_save() {
        let dir = testdir::fresh("settings");
        let path = dir.join("settings.json");
        assert_eq!(Settings::load(&path), Settings::default());
        let s = Settings::default()
            .patched(&json!({"keepCookies": true}))
            .unwrap();
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path), s);
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"version\": 1"));
    }
}
