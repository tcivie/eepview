// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Site icons: the sanitizer and the store (`docs/wiki/site-icons.md`).
//!
//! [`sanitize`] accepts PNG, ICO, GIF, JPEG and WebP by their magic bytes only, decodes the
//! image, and draws it again as two new PNG files. No byte of the original file reaches the
//! UI or the disk. [`IconStore`] keeps those files under a SHA-256 of the host, never the
//! host itself. The request lives in `net::icons`.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::{self, Write as _};
use std::fs;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use image::codecs::png::PngEncoder;
use image::imageops::{self, FilterType};
use image::{DynamicImage, ExtendedColorType, ImageEncoder, ImageError, ImageFormat, RgbaImage};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::store::{VERSION, read_json, write_json};

/// The small icon: tabs and history rows.
pub const SMALL_PX: u32 = 32;
/// The large icon: bookmarks.
pub const LARGE_PX: u32 = 64;
/// The widest or tallest source image accepted.
pub const MAX_SOURCE_PX: u32 = 1024;
/// eepview asks a host at most once in this time.
pub const REFETCH_MS: u64 = 24 * 60 * 60 * 1000;
/// Most fetches at the same time.
pub const MAX_IN_FLIGHT: usize = 2;
/// The start of every icon the UI gets.
pub const DATA_URL_PREFIX: &str = "data:image/png;base64,";

/// Most bytes the decoder may allocate.
const MAX_ALLOC: u64 = 64 * 1024 * 1024;
/// The attempt times, next to the icons.
const ATTEMPTS: &str = "attempts.json";

/// An accepted image type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// PNG.
    Png,
    /// Windows icon.
    Ico,
    /// GIF.
    Gif,
    /// JPEG.
    Jpeg,
    /// WebP.
    WebP,
}

/// One accepted type and its magic bytes.
type Magic = (Format, &'static [(usize, &'static [u8])]);

/// Magic bytes: every `(offset, bytes)` pair must match.
const MAGIC: [Magic; 6] = [
    (Format::Png, &[(0, b"\x89PNG\r\n\x1a\n")]),
    (Format::Ico, &[(0, &[0, 0, 1, 0])]),
    (Format::Gif, &[(0, b"GIF87a")]),
    (Format::Gif, &[(0, b"GIF89a")]),
    (Format::Jpeg, &[(0, &[0xFF, 0xD8, 0xFF])]),
    (Format::WebP, &[(0, b"RIFF"), (8, b"WEBP")]),
];

/// The type from the magic bytes, or `None`.
#[must_use]
pub fn sniff(bytes: &[u8]) -> Option<Format> {
    MAGIC
        .iter()
        .find(|(_, parts)| {
            parts
                .iter()
                .all(|(at, magic)| bytes.get(*at..).is_some_and(|b| b.starts_with(magic)))
        })
        .map(|(format, _)| *format)
}

/// Two new PNG files, drawn from the pixels of an accepted image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Icon {
    /// [`SMALL_PX`] square.
    pub small: Vec<u8>,
    /// [`LARGE_PX`] square.
    pub large: Vec<u8>,
}

/// Why an image was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SanitizeError {
    /// The magic bytes are not an accepted type.
    Format,
    /// The image does not decode.
    Decode,
    /// The image is larger than [`MAX_SOURCE_PX`].
    TooLarge,
    /// The new PNG could not be written.
    Encode,
}

impl fmt::Display for SanitizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Format => "not a PNG, ICO, GIF, JPEG or WebP image",
            Self::Decode => "the image does not decode",
            Self::TooLarge => "the image is too large",
            Self::Encode => "the icon could not be written",
        })
    }
}

impl std::error::Error for SanitizeError {}

/// Checks, decodes and draws an image again as two new PNG files.
///
/// # Errors
///
/// See [`SanitizeError`].
pub fn sanitize(bytes: &[u8]) -> Result<Icon, SanitizeError> {
    let format = sniff(bytes).ok_or(SanitizeError::Format)?;
    let image = decode(bytes, format)?;
    Ok(Icon {
        small: render(&image, SMALL_PX)?,
        large: render(&image, LARGE_PX)?,
    })
}

fn image_format(format: Format) -> ImageFormat {
    match format {
        Format::Png => ImageFormat::Png,
        Format::Ico => ImageFormat::Ico,
        Format::Gif => ImageFormat::Gif,
        Format::Jpeg => ImageFormat::Jpeg,
        Format::WebP => ImageFormat::WebP,
    }
}

/// Decodes with the type from the magic bytes and the size limits set before decoding.
fn decode(bytes: &[u8], format: Format) -> Result<DynamicImage, SanitizeError> {
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), image_format(format));
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SOURCE_PX);
    limits.max_image_height = Some(MAX_SOURCE_PX);
    limits.max_alloc = Some(MAX_ALLOC);
    reader.limits(limits);
    let image = reader.decode().map_err(|e| match e {
        ImageError::Limits(_) => SanitizeError::TooLarge,
        _ => SanitizeError::Decode,
    })?;
    let (w, h) = (image.width(), image.height());
    if w == 0 || h == 0 {
        return Err(SanitizeError::Decode);
    }
    if w > MAX_SOURCE_PX || h > MAX_SOURCE_PX {
        return Err(SanitizeError::TooLarge);
    }
    Ok(image)
}

/// `side` scaled by `px / longest`, rounded, at least 1.
fn scaled(side: u32, longest: u32, px: u32) -> u32 {
    let longest = u64::from(longest.max(1));
    let value = (u64::from(side) * u64::from(px) + longest / 2) / longest;
    u32::try_from(value).unwrap_or(px).clamp(1, px)
}

/// The image fitted into a transparent `px` square, centred, as a new PNG.
fn render(image: &DynamicImage, px: u32) -> Result<Vec<u8>, SanitizeError> {
    let rgba = image.to_rgba8();
    let (w, h) = rgba.dimensions();
    let longest = w.max(h);
    let (nw, nh) = (scaled(w, longest, px), scaled(h, longest, px));
    let resized = imageops::resize(&rgba, nw, nh, FilterType::Triangle);
    let mut canvas = RgbaImage::new(px, px);
    let (x, y) = ((px - nw) / 2, (px - nh) / 2);
    imageops::overlay(&mut canvas, &resized, i64::from(x), i64::from(y));
    let mut out = Vec::new();
    PngEncoder::new(&mut out)
        .write_image(canvas.as_raw(), px, px, ExtendedColorType::Rgba8)
        .map_err(|_| SanitizeError::Encode)?;
    Ok(out)
}

/// `data:image/png;base64,<base64>`.
#[must_use]
pub fn data_url(png: &[u8]) -> String {
    format!("{DATA_URL_PREFIX}{}", STANDARD.encode(png))
}

/// 64 lower-case hex characters: the SHA-256 of `host`.
#[must_use]
pub fn file_stem(host: &str) -> String {
    Sha256::digest(host.as_bytes())
        .iter()
        .fold(String::with_capacity(64), |mut hex, b| {
            let _ = write!(hex, "{b:02x}");
            hex
        })
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct AttemptsFile {
    version: u32,
    attempts: BTreeMap<String, u64>,
}

/// The icons on disk, or in memory only (`IconStore::default()`).
#[derive(Debug, Default)]
pub struct IconStore {
    dir: Option<PathBuf>,
    /// Last attempt per file stem, Unix ms.
    attempts: BTreeMap<String, u64>,
    /// Icons per file stem, when there is no folder.
    memory: HashMap<String, Icon>,
}

impl IconStore {
    /// Opens `dir` (the `icons/` folder). Missing or broken files read as empty.
    #[must_use]
    pub fn load(dir: &Path) -> Self {
        let attempts = read_json::<AttemptsFile>(&dir.join(ATTEMPTS))
            .map(|f| f.attempts)
            .unwrap_or_default();
        Self {
            dir: Some(dir.to_owned()),
            attempts,
            memory: HashMap::new(),
        }
    }

    fn files(dir: &Path, stem: &str) -> (PathBuf, PathBuf) {
        (Self::file(dir, stem, true), Self::file(dir, stem, false))
    }

    fn file(dir: &Path, stem: &str, small: bool) -> PathBuf {
        let suffix = if small { "-32" } else { "" };
        dir.join(format!("{stem}{suffix}.png"))
    }

    /// Writes both files of `host`.
    ///
    /// # Errors
    ///
    /// Fails when a file cannot be written.
    pub fn put(&mut self, host: &str, icon: &Icon) -> io::Result<()> {
        let stem = file_stem(host);
        let Some(dir) = &self.dir else {
            self.memory.insert(stem, icon.clone());
            return Ok(());
        };
        fs::create_dir_all(dir)?;
        let (small, large) = Self::files(dir, &stem);
        write_atomic(&small, &icon.small)?;
        write_atomic(&large, &icon.large)
    }

    /// The 32 px icon of `host` as a data URL.
    #[must_use]
    pub fn small(&self, host: &str) -> Option<String> {
        self.read(host, true)
    }

    /// The 64 px icon of `host` as a data URL.
    #[must_use]
    pub fn large(&self, host: &str) -> Option<String> {
        self.read(host, false)
    }

    fn read(&self, host: &str, small: bool) -> Option<String> {
        let stem = file_stem(host);
        let bytes = match &self.dir {
            None => self.memory.get(&stem).map(|i| pick(i, small).clone())?,
            Some(dir) => fs::read(Self::file(dir, &stem, small)).ok()?,
        };
        (sniff(&bytes) == Some(Format::Png)).then(|| data_url(&bytes))
    }

    /// The last attempt for `host`, Unix ms.
    #[must_use]
    pub fn last_attempt(&self, host: &str) -> Option<u64> {
        self.attempts.get(&file_stem(host)).copied()
    }

    /// Records an attempt for `host` and saves the attempt times.
    ///
    /// # Errors
    ///
    /// Fails when the file cannot be written.
    pub fn record_attempt(&mut self, host: &str, now: u64) -> io::Result<()> {
        self.attempts.insert(file_stem(host), now);
        self.save_attempts()
    }

    fn save_attempts(&self) -> io::Result<()> {
        let Some(dir) = &self.dir else {
            return Ok(());
        };
        let file = AttemptsFile {
            version: VERSION,
            attempts: self.attempts.clone(),
        };
        write_json(&dir.join(ATTEMPTS), &file)
    }

    /// Deletes the icons, attempt times and stray files of every host not in `live`.
    /// True when something was deleted.
    ///
    /// # Errors
    ///
    /// Fails when a file cannot be deleted or the attempt times cannot be saved.
    pub fn retain(&mut self, live: &[String]) -> io::Result<bool> {
        let keep: BTreeSet<String> = live.iter().map(|h| file_stem(h)).collect();
        let before = (self.attempts.len(), self.memory.len());
        self.attempts.retain(|stem, _| keep.contains(stem));
        self.memory.retain(|stem, _| keep.contains(stem));
        let mut changed = before != (self.attempts.len(), self.memory.len());
        if let Some(dir) = self.dir.clone() {
            changed |= sweep(&dir, &keep)?;
        }
        if changed {
            self.save_attempts()?;
        }
        Ok(changed)
    }
}

/// The small or the large PNG of an icon.
fn pick(icon: &Icon, small: bool) -> &Vec<u8> {
    if small { &icon.small } else { &icon.large }
}

/// True for a file name eepview writes for one of the `keep` stems.
fn is_kept(name: &str, keep: &BTreeSet<String>) -> bool {
    if name == ATTEMPTS {
        return true;
    }
    let stem = name
        .strip_suffix("-32.png")
        .or_else(|| name.strip_suffix(".png"));
    stem.is_some_and(|s| keep.contains(s))
}

/// Deletes every entry of `dir` that [`is_kept`] refuses. True when one was deleted.
fn sweep(dir: &Path, keep: &BTreeSet<String>) -> io::Result<bool> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    let mut changed = false;
    for entry in entries {
        let entry = entry?;
        if is_kept(&entry.file_name().to_string_lossy(), keep) {
            continue;
        }
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
        changed = true;
    }
    Ok(changed)
}

/// Writes `bytes` to `path` through a temp file and a rename.
fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("png.tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}
