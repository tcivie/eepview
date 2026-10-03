// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Per-site zoom and JavaScript choices, keyed by host.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::settings::clamp_zoom;
use super::{VERSION, read_json, write_json};

/// The zoom steps of zoom in and zoom out.
const ZOOM_STEPS: [f64; 13] = [
    0.3, 0.5, 0.67, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0,
];

/// Per-site choices.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SitePrefs {
    #[serde(default)]
    zoom: BTreeMap<String, f64>,
    #[serde(default)]
    js: BTreeMap<String, bool>,
}

#[derive(Serialize, Deserialize)]
struct File {
    version: u32,
    #[serde(flatten)]
    prefs: SitePrefs,
}

impl SitePrefs {
    /// Reads the file, or starts empty.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        read_json::<File>(path).map_or_else(Self::default, |f| f.prefs)
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
                prefs: self.clone(),
            },
        )
    }

    /// The zoom of a host, or `default`.
    #[must_use]
    pub fn zoom(&self, host: &str, default: f64) -> f64 {
        self.zoom.get(host).copied().unwrap_or(default)
    }

    /// Remembers a zoom; the default zoom removes the entry.
    pub fn set_zoom(&mut self, host: &str, zoom: f64, default: f64) {
        let zoom = clamp_zoom(zoom);
        if (zoom - default).abs() < 1e-6 {
            self.zoom.remove(host);
        } else {
            self.zoom.insert(host.to_owned(), zoom);
        }
    }

    /// Whether page JavaScript runs on a host.
    #[must_use]
    pub fn js(&self, host: &str, default: bool) -> bool {
        self.js.get(host).copied().unwrap_or(default)
    }

    /// Remembers a JavaScript choice; the default removes the entry.
    pub fn set_js(&mut self, host: &str, on: bool, default: bool) {
        if on == default {
            self.js.remove(host);
        } else {
            self.js.insert(host.to_owned(), on);
        }
    }
}

/// The next zoom step up (`dir` > 0) or down from `current`.
#[must_use]
pub fn zoom_step(current: f64, up: bool) -> f64 {
    let next = if up {
        ZOOM_STEPS.iter().copied().find(|z| *z > current + 1e-6)
    } else {
        ZOOM_STEPS
            .iter()
            .rev()
            .copied()
            .find(|z| *z < current - 1e-6)
    };
    next.unwrap_or(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::testdir;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn zoom_steps() {
        assert!(close(zoom_step(1.0, true), 1.1));
        assert!(close(zoom_step(1.0, false), 0.9));
        assert!(close(zoom_step(3.0, true), 3.0));
        assert!(close(zoom_step(0.3, false), 0.3));
        assert!(close(zoom_step(1.05, true), 1.1));
    }

    #[test]
    fn per_host_values() {
        let mut p = SitePrefs::default();
        assert!(close(p.zoom("a.i2p", 1.0), 1.0));
        p.set_zoom("a.i2p", 1.5, 1.0);
        assert!(close(p.zoom("a.i2p", 1.0), 1.5));
        p.set_zoom("a.i2p", 1.0, 1.0);
        assert!(p.zoom.is_empty());
        assert!(p.js("a.i2p", true));
        p.set_js("a.i2p", false, true);
        assert!(!p.js("a.i2p", true));
        p.set_js("a.i2p", true, true);
        assert!(p.js.is_empty());
    }

    #[test]
    fn load_and_save() {
        let dir = testdir::fresh("prefs");
        let path = dir.join("sites.json");
        assert_eq!(SitePrefs::load(&path), SitePrefs::default());
        let mut p = SitePrefs::default();
        p.set_js("a.i2p", false, true);
        p.set_zoom("b.i2p", 2.0, 1.0);
        p.save(&path).unwrap();
        assert_eq!(SitePrefs::load(&path), p);
    }
}
