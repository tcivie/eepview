<!--
SPDX-FileCopyrightText: 2026 The eepview contributors
SPDX-License-Identifier: MIT
-->

# Site icons

Status: in progress, branch `feat/site-icons`.

Tabs, bookmark tiles, the bookmarks page and history rows show the icon of each site. When a site has no icon, they show the first letter of its host, as before.

## How it works

1. A web tab finishes loading a page on the host `H`.
2. When `H` has a bookmark or a history entry, and eepview has not asked `H` in the last 24 h, the shell asks for an icon.
3. `net::icons` sends one `GET http://H/favicon.ico` to the gatekeeper on loopback. The gatekeeper checks the host as for every page request and forwards it to the router.
4. `icons::sanitize` checks the magic bytes, decodes the image, and draws it again as a 32 x 32 and a 64 x 64 PNG. No byte of the original file reaches the UI.
5. The store writes the two PNG files under the app data folder. The file name is a hash of the host.
6. The shell sends `icons-changed` and `tabs-changed`. The UI reads the icons from `TabInfo.icon`, `Bookmark.icon` and `HistoryEntry.icon`.

## Requirements

Each requirement is a test target. The tests check these rules, not the code.

### What is fetched, and from where

- **R1.** The only request is `GET http://H/favicon.ico` for the host `H` of the page that finished loading. eepview does not read `<link rel="icon">` from the page.
- **R2.** The request goes only to the gatekeeper (`Gatekeeper`, on loopback). It never goes directly to the router proxy, and never to any other address.
- **R3.** The request head is exactly these lines, in this order, and nothing else:
  `GET http://H/favicon.ico HTTP/1.1`, `Host: H`, `Accept: image/*`, `Connection: close`.
  There is no `Cookie`, `Referer`, `User-Agent`, `Origin` or `Accept-Encoding` header.
- **R4.** A host that fails `is_i2p_host` gets no request: no connection is opened.
- **R5.** A redirect (any 3xx) is not followed. Only status 200 is accepted. The icon always comes from `H` itself.

### When

- **R6.** A fetch starts only after a web tab reports that a page on `H` finished loading. Internal pages, blocked pages and pages that never finish start no fetch.
- **R7.** A fetch starts only when `H` has a bookmark, or a history entry after this visit is recorded. With history off and no bookmark for `H`, nothing is fetched and nothing is stored.
- **R8.** eepview asks a host at most once in 24 h, whether the earlier attempt worked or not. The time of the last attempt per host survives a restart.
- **R9.** After 24 h, the next finished page load on `H` asks again. When that attempt fails, the stored icon stays.
- **R10.** At most 2 fetches run at the same time. Other hosts wait in a queue, in order, each host at most once. A waiting host starts when a running fetch ends.
- **R11.** Without a gatekeeper (router not verified, or paused), no fetch is sent. The attempt counts as a failure.

### Limits

- **R12.** A fetch ends after 30 s in total. A fetch that times out stores nothing, even when some bytes arrived.
- **R13.** The body may have at most 64 KiB (65 536 bytes). A larger `Content-Length` is refused before the body is read. A body that grows past the limit is refused.
- **R14.** A response with `Transfer-Encoding` (for example `chunked`) is refused. A body shorter than its `Content-Length` is refused.
- **R15.** A response head larger than 64 KiB, or a response that is not HTTP, is refused.

### Sanitizing

- **R16.** The type comes only from the magic bytes. `Content-Type` is ignored. Accepted types:
  PNG (`89 50 4E 47 0D 0A 1A 0A`), ICO (`00 00 01 00`), GIF (`GIF87a`, `GIF89a`), JPEG (`FF D8 FF`) and WebP (`RIFF`, 4 bytes, `WEBP`).
- **R17.** Every other type is refused, SVG included (also SVG with a PNG `Content-Type`), and so are HTML, BMP, TIFF and empty bodies.
- **R18.** A file with accepted magic bytes that does not decode is refused.
- **R19.** An image wider or taller than 1024 px is refused.
- **R20.** An accepted image is drawn again as two new PNG files: 32 x 32 and 64 x 64, RGBA. A non-square image keeps its shape: it is centred, and the rest is transparent. For an animated GIF, only the first frame is used.
- **R21.** The output is a new PNG made from pixels only. It never contains the input bytes, its metadata or its chunks.

### Storage

- **R22.** Icons live in the folder `icons/` under the app data folder: `icons/<sha256(H)>.png` (64 px) and `icons/<sha256(H)>-32.png` (32 px). `<sha256(H)>` is 64 lower-case hex characters of the SHA-256 of the host text. The host never appears in a file name.
- **R23.** The attempt times live in `icons/attempts.json`, keyed by the same hash. The host never appears in this file.
- **R24.** `bookmarks.json`, `history.json` and the bookmark export never contain icon data. The `icon` field there is absent or `null`.
- **R25.** A failed fetch never deletes or changes an icon that is already stored.

### Clearing

- **R26.** An icon, its attempt time and its files are kept only while `H` has at least one bookmark or history entry. eepview deletes them as soon as the last one goes: `history_remove`, `history_clear` (any range), `bookmark_remove`, and `bookmark_update` that moves a bookmark to another host.
- **R27.** `history_clear("all")` (clear browsing data) deletes the icon of every host that has no bookmark.
- **R28.** At start, eepview deletes every file in `icons/` that belongs to no bookmarked or visited host, and every file it did not write.
- **R29.** After a host is cleared, the next visit asks for its icon again (the 24 h rule starts fresh).
- **R30.** Every change to the stored icons sends `icons-changed` (payload `null`) and `tabs-changed`.

### UI

- **R31.** `TabInfo.icon` and `HistoryEntry.icon` hold the 32 px icon, `Bookmark.icon` the 64 px icon, as `data:image/png;base64,<base64>`. Without a stored icon the value is `null`. Internal tabs always have `null`.
- **R32.** The UI shows an icon only when the value starts with `data:image/png;base64,` and the rest is base64. Anything else shows the letter chip.
- **R33.** Tabs, bookmark tiles, bookmark rows and history rows show the icon as an `<img>` with an empty `alt`. Without an icon they show the letter chip in a neutral style.
- **R34.** The UI reloads the icons on `icons-changed`.
- **R35.** The internal pages load images only from the bundle and from `data:` URLs. The Content-Security-Policy `img-src` has no `http:`, `https:` or host source.
- **R36.** `tab-*` webviews get no IPC. The icon feature adds no capability and no command.

### Architecture

- **R37.** Only `src/net/` opens a socket for icons. `net::icons` connects only through the `Gatekeeper` it receives; it builds no `LoopbackAddr` and names no host other than `.i2p` hosts.
- **R38.** Only the sanitizer (`src/icons.rs`) decodes images.

## Interface

### Rust: `net::icons` (`src-tauri/src/net/icons.rs`)

```rust
pub const PATH: &str = "/favicon.ico";
pub const MAX_BODY: usize = 64 * 1024;
pub const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits { pub timeout: Duration, pub max_body: usize }
impl Limits { pub const DEFAULT: Self; } // TIMEOUT, MAX_BODY

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    NotI2p,        // R4: no connection opened
    Io(String),    // connect, read or write failed (gatekeeper closed, ...)
    Status(u16),   // anything but 200 (R5); the gatekeeper's own 403 or 502 too
    TooLarge,      // R13, R15
    Chunked,       // R14: Transfer-Encoding present
    Malformed,     // not HTTP, or body shorter than Content-Length (R14, R15)
    Timeout,       // R12
}

/// The exact request bytes for `host` (R3), or `None` when `host` is not an I2P host.
pub fn request(host: &str) -> Option<Vec<u8>>;
/// One fetch through the gatekeeper with `Limits::DEFAULT`. Returns the raw body.
pub fn fetch(gate: &Gatekeeper, host: &str) -> Result<Vec<u8>, FetchError>;
/// The same with other limits (tests use a short timeout).
pub fn fetch_with(gate: &Gatekeeper, host: &str, limits: Limits) -> Result<Vec<u8>, FetchError>;
```

`Gatekeeper` gets `pub(crate) fn addr(&self) -> LoopbackAddr`.

### Rust: `icons` (`src-tauri/src/icons.rs`)

```rust
pub const SMALL_PX: u32 = 32;
pub const LARGE_PX: u32 = 64;
pub const MAX_SOURCE_PX: u32 = 1024;
pub const REFETCH_MS: u64 = 24 * 60 * 60 * 1000;
pub const MAX_IN_FLIGHT: usize = 2;
pub const DATA_URL_PREFIX: &str = "data:image/png;base64,";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format { Png, Ico, Gif, Jpeg, WebP }
/// The type from the magic bytes (R16), or `None`.
pub fn sniff(bytes: &[u8]) -> Option<Format>;

/// Two new PNG files (R20).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Icon { pub small: Vec<u8>, pub large: Vec<u8> }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SanitizeError { Format, Decode, TooLarge, Encode }
pub fn sanitize(bytes: &[u8]) -> Result<Icon, SanitizeError>;

/// `data:image/png;base64,<standard base64 with padding>`.
pub fn data_url(png: &[u8]) -> String;
/// 64 lower-case hex characters: SHA-256 of `host` (R22).
pub fn file_stem(host: &str) -> String;

/// The icons on disk (or in memory only, for `IconStore::default()`).
#[derive(Debug, Default)]
pub struct IconStore { /* private */ }
impl IconStore {
    /// Opens `dir` (the `icons/` folder). Missing or broken files read as empty.
    pub fn load(dir: &Path) -> Self;
    /// Writes both files of `host` (R22).
    pub fn put(&mut self, host: &str, icon: &Icon) -> io::Result<()>;
    /// The 32 px icon as a data URL.
    pub fn small(&self, host: &str) -> Option<String>;
    /// The 64 px icon as a data URL.
    pub fn large(&self, host: &str) -> Option<String>;
    /// The last attempt for `host`, Unix ms (R8).
    pub fn last_attempt(&self, host: &str) -> Option<u64>;
    /// Records an attempt and saves `attempts.json` (R23).
    pub fn record_attempt(&mut self, host: &str, now: u64) -> io::Result<()>;
    /// Deletes the icons, attempt times and stray files of every host not in `live` (R26, R28).
    /// True when something was deleted.
    pub fn retain(&mut self, live: &[String]) -> io::Result<bool>;
}
```

### Rust: core and shell

```rust
pub struct Paths { /* ... */ pub icons: PathBuf } // <app data>/icons

pub enum Effect { /* ... */ FetchIcon(String) }   // the host; the shell runs the fetch
pub enum Event { /* ... */ Icons }                 // "icons-changed"

impl Core {
    /// A fetch for `host` ended: `Some` with the sanitized icon, `None` on any failure.
    /// Stores it, sends Icons and TabsChanged, and starts the next waiting host (R10).
    pub fn icon_fetched(&mut self, host: &str, icon: Option<Icon>, now: u64) -> Vec<Effect>;
}
```

`Core::page_finished` returns `Effect::FetchIcon(H)` when R6 to R11 allow it. The shell runs `net::icons::fetch` and `icons::sanitize` on a worker thread, then calls `Core::icon_fetched`.

### IPC (contract v1.4)

```ts
type TabInfo = { /* ... */ icon: string | null };
type Bookmark = { /* ... */ icon: string | null };
type HistoryEntry = { /* ... */ icon: string | null };
// event
"icons-changed": null
```

`bookmark_update` and `bookmarks_import` ignore an `icon` they receive.

### UI: `src/ui/lib/site-icon.ts`

```ts
export const ICON_PREFIX = "data:image/png;base64,";
export type SiteMark = { kind: "icon"; src: string } | { kind: "letter"; text: string };
/** The icon when it is a valid PNG data URL (R32), or the letter chip. */
export function siteMark(icon: string | null | undefined, letter: string): SiteMark;
```

## Limits

- Only `/favicon.ico`. A site that names its icon only in `<link rel="icon">` shows the letter.
- A chunked or compressed icon response is refused.

## History

- 2026-10-03 — Requirements and interface — branch `feat/site-icons`
