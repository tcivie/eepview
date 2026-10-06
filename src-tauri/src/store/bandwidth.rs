// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The saved router bandwidth (`bandwidth.json` in the app data folder): the samples of the
//! last 10 minutes, as numbers and times only. No URL, no host, no source.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{read_json, write_json};
use crate::net::stats::{History, Sample};

/// The schema version of `bandwidth.json`.
pub const VERSION: u64 = 1;

/// The file content.
#[derive(Serialize, Deserialize)]
struct File {
    version: u64,
    samples: Vec<Sample>,
}

/// The history of `path`, restored at `now` ([`History::restored`]). Empty when the file is
/// missing, broken or of another version.
#[must_use]
pub fn load(path: &Path, now: u64) -> History {
    read_json::<File>(path)
        .filter(|file| file.version == VERSION)
        .map_or_else(History::default, |file| {
            History::restored(now, file.samples)
        })
}

/// Writes the history to `path` atomically (temp file + rename).
///
/// # Errors
///
/// Fails when the folder cannot be made or the file cannot be written.
pub fn save(path: &Path, history: &History) -> io::Result<()> {
    let file = File {
        version: VERSION,
        samples: history.samples(),
    };
    write_json(path, &file)
}

#[cfg(test)]
mod tests;
