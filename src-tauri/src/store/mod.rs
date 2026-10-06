// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Small JSON stores with a `version` field, written atomically (temp file + rename).

pub mod bandwidth;
pub mod bookmarks;
pub mod history;
pub mod settings;
pub mod site_prefs;

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::diag::{self, Code, ErrorKind, Field, StoreKind};

/// The schema version every store writes.
pub const VERSION: u32 = 1;

/// Reads a JSON file. `None` when it is missing or broken (the caller starts fresh). A
/// broken file records `store-corrupt` with the store and the error kind.
#[must_use]
pub fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return None,
        Err(e) => return corrupt(path, ErrorKind::from(&e)),
    };
    serde_json::from_str(&text)
        .map_err(|e| corrupt::<()>(path, ErrorKind::from(&e)))
        .ok()
}

/// Records a broken store file and gives `None`.
fn corrupt<T>(path: &Path, error: ErrorKind) -> Option<T> {
    let mut fields = vec![Field::Error(error)];
    fields.extend(store_kind(path).map(Field::Store));
    diag::event(Code::StoreCorrupt, &fields);
    None
}

/// The store a file belongs to, by its name.
#[must_use]
pub fn store_kind(path: &Path) -> Option<StoreKind> {
    match path.file_stem()?.to_str()? {
        "bookmarks" => Some(StoreKind::Bookmarks),
        "history" => Some(StoreKind::History),
        "settings" => Some(StoreKind::Settings),
        "sites" => Some(StoreKind::Sites),
        "bandwidth" => Some(StoreKind::Bandwidth),
        _ => None,
    }
}

/// Writes `value` as JSON to `path` atomically: a temp file next to it, flushed, then renamed.
///
/// # Errors
///
/// Fails when the directory cannot be created or the file cannot be written.
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    let mut file = fs::File::create(&tmp)?;
    file.write_all(&text)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&tmp, path)
}

/// A short unique id from a time in ms and a sequence number.
#[must_use]
pub fn new_id(now: u64, seq: u64) -> String {
    format!("{now:x}-{seq:x}")
}

#[cfg(test)]
pub(crate) mod testdir {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT: AtomicU32 = AtomicU32::new(0);

    /// A fresh empty directory under the system temp dir.
    pub fn fresh(name: &str) -> PathBuf {
        let n = NEXT.fetch_add(1, Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("eepview-test-{}-{name}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn round_trip_and_atomic_replace() {
        let dir = testdir::fresh("store");
        let path = dir.join("sub").join("a.json");
        write_json(&path, &json!({"version": 1, "x": 1})).unwrap();
        write_json(&path, &json!({"version": 1, "x": 2})).unwrap();
        let back: Value = read_json(&path).unwrap();
        assert_eq!(back["x"], 2);
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn missing_or_broken_files_read_as_none() {
        let dir = testdir::fresh("broken");
        assert!(read_json::<Value>(&dir.join("none.json")).is_none());
        std::fs::write(dir.join("settings.json"), "{nope").unwrap();
        assert!(read_json::<Value>(&dir.join("settings.json")).is_none());
        assert!(read_json::<Value>(&dir).is_none());
        assert_eq!(store_kind(&dir.join("sites.json")), Some(StoreKind::Sites));
        assert_eq!(store_kind(&dir.join("other.json")), None);
    }

    #[test]
    fn write_fails_on_a_file_in_place_of_the_dir() {
        let dir = testdir::fresh("blocked");
        std::fs::write(dir.join("file"), "x").unwrap();
        assert!(write_json(&dir.join("file").join("a.json"), &1).is_err());
    }

    #[test]
    fn ids_are_distinct() {
        assert_ne!(new_id(1, 1), new_id(1, 2));
        assert_eq!(new_id(255, 16), "ff-10");
    }
}
