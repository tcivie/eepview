//! Bookmarks: one level of folders, JSON import and export. Pure logic plus file I/O.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{VERSION, new_id, read_json, write_json};
use crate::nav::{Target, classify};
use crate::types::{Bookmark, NewBookmark};

/// The first-run bookmarks.
const SEED: [(&str, &str); 4] = [
    ("http://stats.i2p/", "stats.i2p"),
    ("http://i2p-projekt.i2p/", "The Invisible Internet Project"),
    ("http://reg.i2p/", "reg.i2p"),
    ("http://notbob.i2p/", "notbob.i2p"),
];

#[derive(Debug, Serialize, Deserialize)]
struct File {
    version: u32,
    bookmarks: Vec<Bookmark>,
}

/// The import format: the export file, or a bare list.
#[derive(Deserialize)]
#[serde(untagged)]
enum Import {
    File { bookmarks: Vec<Bookmark> },
    List(Vec<Bookmark>),
}

/// All bookmarks, in the order the user made them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bookmarks {
    items: Vec<Bookmark>,
    seq: u64,
}

/// The address a bookmark may hold: an I2P site or an internal page, normalised.
#[must_use]
pub fn normalise(url: &str) -> Option<String> {
    match classify(url) {
        Target::Web(u) => Some(u.to_string()),
        Target::Internal(s) => Some(s),
        Target::Search(_) | Target::Refused(_) => None,
    }
}

impl Bookmarks {
    /// The seeded first-run list.
    #[must_use]
    pub fn seeded(now: u64) -> Self {
        let mut store = Self::default();
        for (url, title) in SEED {
            let _ = store.add(
                &NewBookmark {
                    url: url.into(),
                    title: title.into(),
                    folder: None,
                },
                now,
            );
        }
        store
    }

    /// Reads the file, or seeds a new list when there is none.
    #[must_use]
    pub fn load(path: &Path, now: u64) -> Self {
        read_json::<File>(path).map_or_else(
            || Self::seeded(now),
            |f| Self {
                seq: f.bookmarks.len() as u64,
                items: f.bookmarks,
            },
        )
    }

    /// Writes the file.
    ///
    /// # Errors
    ///
    /// Fails when the file cannot be written.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        write_json(path, &self.file())
    }

    fn file(&self) -> File {
        File {
            version: VERSION,
            bookmarks: self.items.clone(),
        }
    }

    /// All bookmarks.
    #[must_use]
    pub fn list(&self) -> &[Bookmark] {
        &self.items
    }

    /// Adds a bookmark, or returns the existing one for the same URL.
    ///
    /// # Errors
    ///
    /// Fails when the URL is not an I2P site or an internal page.
    pub fn add(&mut self, new: &NewBookmark, now: u64) -> Result<Bookmark, String> {
        let url = normalise(&new.url).ok_or_else(|| format!("not an I2P address: {}", new.url))?;
        if let Some(existing) = self.find(&url) {
            return Ok(existing.clone());
        }
        self.seq += 1;
        let title = if new.title.trim().is_empty() {
            url.clone()
        } else {
            new.title.trim().to_owned()
        };
        let bookmark = Bookmark {
            id: new_id(now, self.seq),
            url,
            title,
            folder: clean_folder(new.folder.as_deref()),
            created: now,
        };
        self.items.push(bookmark.clone());
        Ok(bookmark)
    }

    /// Replaces the bookmark with the same id. False when there is none or the URL is bad.
    pub fn update(&mut self, bookmark: &Bookmark) -> bool {
        let Some(url) = normalise(&bookmark.url) else {
            return false;
        };
        let Some(slot) = self.items.iter_mut().find(|b| b.id == bookmark.id) else {
            return false;
        };
        slot.url = url;
        slot.title.clone_from(&bookmark.title);
        slot.folder = clean_folder(bookmark.folder.as_deref());
        true
    }

    /// Removes a bookmark. False when there is none.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|b| b.id != id);
        self.items.len() != before
    }

    /// The bookmark of a URL, compared after normalisation.
    #[must_use]
    pub fn find(&self, url: &str) -> Option<&Bookmark> {
        let url = normalise(url).unwrap_or_else(|| url.to_owned());
        self.items.iter().find(|b| b.url == url)
    }

    /// The export file as JSON text.
    #[must_use]
    pub fn export(&self) -> String {
        serde_json::to_string_pretty(&self.file()).unwrap_or_default()
    }

    /// Imports bookmarks from JSON; skips bad URLs and URLs already saved. Returns the count added.
    ///
    /// # Errors
    ///
    /// Fails when the text is not an export file or a list of bookmarks.
    pub fn import(&mut self, json: &str, now: u64) -> Result<usize, String> {
        let list = match serde_json::from_str::<Import>(json).map_err(|e| e.to_string())? {
            Import::File { bookmarks } | Import::List(bookmarks) => bookmarks,
        };
        let before = self.items.len();
        for b in list {
            let new = NewBookmark {
                url: b.url,
                title: b.title,
                folder: b.folder,
            };
            let _ = self.add(&new, now);
        }
        Ok(self.items.len() - before)
    }
}

/// The export file name for a time in Unix ms: `eepview-bookmarks-<YYYYMMDD>.json` (UTC).
#[must_use]
pub fn export_file_name(now_ms: u64) -> String {
    let (y, m, d) = civil_date(now_ms / 86_400_000);
    format!("eepview-bookmarks-{y:04}{m:02}{d:02}.json")
}

/// Year, month and day of a count of days since 1970-01-01 (Hinnant's civil-from-days).
#[must_use]
pub fn civil_date(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + u64::from(m <= 2);
    (y, m, d)
}

/// One level of folders: no slashes, no empty names.
fn clean_folder(folder: Option<&str>) -> Option<String> {
    let name = folder?.trim().replace('/', " ");
    (!name.is_empty()).then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::testdir;

    fn new(url: &str) -> NewBookmark {
        NewBookmark {
            url: url.into(),
            title: String::new(),
            folder: None,
        }
    }

    #[test]
    fn seeded_with_four_sites() {
        let b = Bookmarks::seeded(1);
        let urls: Vec<&str> = b.list().iter().map(|x| x.url.as_str()).collect();
        assert_eq!(urls, SEED.map(|(u, _)| u).to_vec());
    }

    #[test]
    fn add_normalises_and_dedups() {
        let mut b = Bookmarks::default();
        let first = b.add(&new("Stats.I2P"), 5).unwrap();
        assert_eq!(first.url, "http://stats.i2p/");
        assert_eq!(first.title, "http://stats.i2p/");
        assert_eq!(first.created, 5);
        let again = b.add(&new("http://stats.i2p/"), 6).unwrap();
        assert_eq!(again.id, first.id);
        assert_eq!(b.list().len(), 1);
        assert!(b.add(&new("http://example.com/"), 7).is_err());
        assert!(b.add(&new("eepview://home"), 7).is_ok());
    }

    #[test]
    fn folders_are_one_level() {
        let mut b = Bookmarks::default();
        let mut n = new("a.i2p");
        n.folder = Some(" news/tech ".into());
        assert_eq!(b.add(&n, 1).unwrap().folder.as_deref(), Some("news tech"));
        n.url = "b.i2p".into();
        n.folder = Some("  ".into());
        assert_eq!(b.add(&n, 1).unwrap().folder, None);
    }

    #[test]
    fn update_remove_find() {
        let mut b = Bookmarks::default();
        let mut x = b.add(&new("a.i2p"), 1).unwrap();
        x.title = "A".into();
        x.url = "b.i2p".into();
        assert!(b.update(&x));
        assert_eq!(b.find("http://b.i2p/").unwrap().title, "A");
        assert!(b.find("a.i2p").is_none());
        x.url = "evil.com".into();
        assert!(!b.update(&x));
        x.url = "c.i2p".into();
        x.id = "none".into();
        assert!(!b.update(&x));
        assert!(b.remove(&b.list()[0].id.clone()));
        assert!(!b.remove("none"));
        assert!(b.find("not a url at all").is_none());
    }

    #[test]
    fn export_import_round_trip() {
        let a = Bookmarks::seeded(1);
        let mut b = Bookmarks::default();
        assert_eq!(b.import(&a.export(), 2).unwrap(), 4);
        assert_eq!(b.import(&a.export(), 2).unwrap(), 0);
        let bare =
            r#"[{"id":"x","url":"new.i2p","title":"N"},{"id":"y","url":"evil.com","title":"E"}]"#;
        assert_eq!(b.import(bare, 3).unwrap(), 1);
        assert!(b.import("{}", 3).is_err());
        assert!(a.export().contains("\"version\": 1"));
    }

    #[test]
    fn export_names() {
        assert_eq!(civil_date(0), (1970, 1, 1));
        assert_eq!(civil_date(59), (1970, 3, 1));
        assert_eq!(civil_date(11_016), (2000, 2, 29));
        assert_eq!(
            export_file_name(1_791_000_000_000),
            "eepview-bookmarks-20261003.json"
        );
    }

    #[test]
    fn load_and_save() {
        let dir = testdir::fresh("bookmarks");
        let path = dir.join("bookmarks.json");
        let fresh = Bookmarks::load(&path, 1);
        assert_eq!(fresh.list().len(), 4);
        let mut b = fresh.clone();
        b.add(&new("x.i2p"), 2).unwrap();
        b.save(&path).unwrap();
        assert_eq!(Bookmarks::load(&path, 3).list().len(), 5);
    }
}
