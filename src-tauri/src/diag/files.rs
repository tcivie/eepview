// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Safe file creation for the diagnostics files (R14): a private folder (0700), private
//! files (0600), and never a write through a symbolic link.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;

/// Creates `dir` and its parents, and makes `dir` private (0700 on Unix).
///
/// # Errors
///
/// Fails when the folder cannot be created.
pub fn private_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    restrict(dir, 0o700)
}

#[cfg(unix)]
fn restrict(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn restrict(_path: &Path, _mode: u32) -> io::Result<()> {
    Ok(())
}

/// Private file options that never follow a symbolic link (0600, `O_NOFOLLOW` on Unix).
fn private() -> OpenOptions {
    let mut options = OpenOptions::new();
    no_follow(&mut options);
    options
}

#[cfg(unix)]
fn no_follow(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
}

#[cfg(not(unix))]
fn no_follow(_options: &mut OpenOptions) {}

/// Opens `path` to append, creating it 0600. A symbolic link makes it fail.
///
/// # Errors
///
/// Fails when the file cannot be opened.
pub fn append(path: &Path) -> io::Result<File> {
    let file = private().create(true).append(true).open(path)?;
    restrict(path, 0o600)?;
    Ok(file)
}

/// Creates or empties `path`, 0600. A symbolic link makes it fail.
///
/// # Errors
///
/// Fails when the file cannot be written.
pub fn replace(path: &Path) -> io::Result<File> {
    let file = private()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    restrict(path, 0o600)?;
    Ok(file)
}

/// Creates a new file at `path`, 0600. It fails when anything, a link included, is there.
///
/// # Errors
///
/// Fails when the path exists or cannot be written.
pub fn create_new(path: &Path) -> io::Result<File> {
    private().create_new(true).write(true).open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn modes_are_private_and_links_are_refused() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let dir = crate::store::testdir::fresh("diag-files-safe").join("logs");
        private_dir(&dir).unwrap();
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&dir), 0o700);
        drop(append(&dir.join("a.log")).unwrap());
        drop(replace(&dir.join("m")).unwrap());
        drop(create_new(&dir.join("r.txt")).unwrap());
        for name in ["a.log", "m", "r.txt"] {
            assert_eq!(mode(&dir.join(name)), 0o600, "{name}");
        }
        symlink(dir.join("a.log"), dir.join("link")).unwrap();
        assert!(append(&dir.join("link")).is_err());
        assert!(replace(&dir.join("link")).is_err());
        assert!(create_new(&dir.join("link")).is_err());
    }
}
