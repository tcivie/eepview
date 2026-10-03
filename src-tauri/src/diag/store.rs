// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Where events go: an in-memory ring of the last events, and three rotating log files in
//! the app log folder. Both stay on this computer.

use std::collections::VecDeque;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::types::Record;

/// The last events, oldest first.
#[derive(Debug, Clone)]
pub struct Ring {
    records: VecDeque<Record>,
    capacity: usize,
}

impl Ring {
    /// The size of the app's ring.
    pub const CAPACITY: usize = 2000;

    /// An empty ring that keeps at most `capacity` records.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            records: VecDeque::with_capacity(capacity.min(Self::CAPACITY)),
            capacity,
        }
    }

    /// Adds a record; a full ring drops its oldest one.
    pub fn push(&mut self, record: Record) {
        if self.capacity == 0 {
            return;
        }
        while self.records.len() >= self.capacity {
            self.records.pop_front();
        }
        self.records.push_back(record);
    }

    /// The number of records.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// True when the ring holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The last `n` records, oldest first.
    #[must_use]
    pub fn last(&self, n: usize) -> Vec<Record> {
        let skip = self.records.len().saturating_sub(n);
        self.records.iter().skip(skip).cloned().collect()
    }

    /// Drops every record.
    pub fn clear(&mut self) {
        self.records.clear();
    }
}

/// The rotating log files: `eepview.log`, `eepview.1.log`, `eepview.2.log`.
#[derive(Debug, Clone)]
pub struct LogFiles {
    dir: PathBuf,
    max_bytes: u64,
}

impl LogFiles {
    /// The largest size of one file.
    pub const MAX_BYTES: u64 = 512 * 1024;
    /// The number of files.
    pub const FILES: usize = 3;

    /// Log files in `dir`, 512 KB each.
    #[must_use]
    pub fn new(dir: PathBuf) -> Self {
        Self::with_limit(dir, Self::MAX_BYTES)
    }

    /// Log files in `dir`, `max_bytes` each.
    #[must_use]
    pub fn with_limit(dir: PathBuf, max_bytes: u64) -> Self {
        Self { dir, max_bytes }
    }

    fn path(&self, index: usize) -> PathBuf {
        if index == 0 {
            self.dir.join("eepview.log")
        } else {
            self.dir.join(format!("eepview.{index}.log"))
        }
    }

    /// Writes `line` and a newline, after a rotation when the file would grow too big.
    ///
    /// # Errors
    ///
    /// Fails when the folder or the file cannot be written.
    pub fn append(&mut self, line: &str) -> io::Result<()> {
        super::files::private_dir(&self.dir)?;
        let current = self.path(0);
        let size = fs::metadata(&current).map_or(0, |m| m.len());
        let added = line.len() as u64 + 1;
        if size > 0 && size + added > self.max_bytes {
            self.rotate()?;
        }
        let mut file = super::files::append(&current)?;
        file.write_all(format!("{line}\n").as_bytes())
    }

    fn rotate(&self) -> io::Result<()> {
        (1..Self::FILES)
            .rev()
            .map(|i| (self.path(i - 1), self.path(i)))
            .filter(|(from, _)| from.exists())
            .try_for_each(|(from, to)| fs::rename(from, to))
    }

    /// The files that exist, newest first.
    #[must_use]
    pub fn paths(&self) -> Vec<PathBuf> {
        (0..Self::FILES)
            .map(|i| self.path(i))
            .filter(|p| p.exists())
            .collect()
    }

    /// Removes every log file.
    ///
    /// # Errors
    ///
    /// Fails when a file exists and cannot be removed.
    pub fn delete_all(&self) -> io::Result<()> {
        for path in self.paths() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    /// The folder.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::types::Code;

    fn record(at: u64) -> Record {
        Record {
            at,
            code: Code::Startup,
            fields: Vec::new(),
        }
    }

    #[test]
    fn a_zero_ring_keeps_nothing() {
        let mut ring = Ring::new(0);
        ring.push(record(1));
        assert!(ring.is_empty());
        let mut ring = Ring::new(2);
        for at in 0..5 {
            ring.push(record(at));
        }
        assert_eq!(
            ring.last(9).iter().map(|r| r.at).collect::<Vec<_>>(),
            [3, 4]
        );
    }

    #[test]
    fn files_rotate_and_go() {
        let dir = crate::store::testdir::fresh("diag-files");
        let mut files = LogFiles::with_limit(dir.join("logs"), 10);
        for _ in 0..5 {
            files.append("123456789").unwrap();
        }
        assert_eq!(files.paths().len(), 3);
        assert_eq!(files.dir(), dir.join("logs"));
        files.delete_all().unwrap();
        assert!(files.paths().is_empty());
    }
}
