// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Site icons, store side (R8, R22, R23, R25, R26, R28 of `docs/wiki/site-icons.md`):
//! file names, `attempts.json`, and `IconStore::retain`.

use std::error::Error;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use eepview_lib::icons::{Icon, IconStore, data_url};
use image::{ImageFormat, Rgba, RgbaImage};

type Res<T> = Result<T, Box<dyn Error>>;

const ALPHA: &str = "alpha.i2p";
const ALPHA_STEM: &str = "3cb7d940c289d5f66ec11ccd02197102e5d1ecf80b0ff3bfb8a491910238ba26";
const BETA: &str = "beta.i2p";
const BETA_STEM: &str = "1c39035de7259a06bf73927f988ba408c7c6cd55981b782b438ae239f23f779f";

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A fresh empty `icons/` folder.
fn fresh(name: &str) -> Res<PathBuf> {
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "eepview-icons-store-{}-{name}-{n}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// A real PNG of `px` x `px` pixels in a colour made from `seed`.
fn png(px: u32, seed: u8) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    let img = RgbaImage::from_pixel(px, px, Rgba([seed, 255 - seed, seed / 2, 255]));
    img.write_to(&mut out, ImageFormat::Png).unwrap_or_default();
    out.into_inner()
}

/// Two distinct icons: a 32 px and a 64 px PNG, different for each seed.
fn icon(seed: u8) -> Icon {
    Icon {
        small: png(32, seed),
        large: png(64, seed.wrapping_add(1)),
    }
}

fn live(hosts: &[&str]) -> Vec<String> {
    hosts.iter().map(|h| (*h).to_owned()).collect()
}

/// The file names in `dir`, sorted.
fn names(dir: &Path) -> Res<Vec<String>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        out.push(entry?.file_name().to_string_lossy().into_owned());
    }
    out.sort();
    Ok(out)
}

#[test]
fn r22_put_writes_the_64_and_the_32_pixel_file_named_by_the_hash() -> Res<()> {
    // R22: icons/<sha256(H)>.png (64 px) and icons/<sha256(H)>-32.png (32 px).
    let dir = fresh("put")?;
    let mut store = IconStore::load(&dir);
    store.put(ALPHA, &icon(7))?;
    assert_eq!(
        fs::read(dir.join(format!("{ALPHA_STEM}.png")))?,
        icon(7).large
    );
    assert_eq!(
        fs::read(dir.join(format!("{ALPHA_STEM}-32.png")))?,
        icon(7).small
    );
    Ok(())
}

#[test]
fn r22_the_host_never_appears_in_a_file_name_or_in_attempts_json() -> Res<()> {
    // R22, R23: only hash names, and the host is not in the attempts file.
    let dir = fresh("names")?;
    let mut store = IconStore::load(&dir);
    store.put(ALPHA, &icon(1))?;
    store.put(BETA, &icon(2))?;
    store.record_attempt(ALPHA, 1_000)?;
    store.record_attempt(BETA, 2_000)?;
    let mut want = vec![
        format!("{ALPHA_STEM}.png"),
        format!("{ALPHA_STEM}-32.png"),
        format!("{BETA_STEM}.png"),
        format!("{BETA_STEM}-32.png"),
        "attempts.json".to_owned(),
    ];
    want.sort();
    assert_eq!(names(&dir)?, want);
    let attempts = fs::read_to_string(dir.join("attempts.json"))?;
    assert!(!attempts.contains("alpha") && !attempts.contains("beta"));
    assert!(!attempts.contains("i2p"), "R23: host text in attempts.json");
    Ok(())
}

#[test]
fn r23_attempts_json_is_json_keyed_by_the_same_hash() -> Res<()> {
    // R23: the key is the hash of the host.
    let dir = fresh("attempts")?;
    let mut store = IconStore::load(&dir);
    store.record_attempt(ALPHA, 123_456)?;
    let text = fs::read_to_string(dir.join("attempts.json"))?;
    let json: serde_json::Value = serde_json::from_str(&text)?;
    assert!(json.to_string().contains(ALPHA_STEM), "{text}");
    assert!(json.to_string().contains("123456"), "{text}");
    Ok(())
}

#[test]
fn r22_small_and_large_return_the_files_as_png_data_urls() -> Res<()> {
    // R22, R31: 32 px for `small`, 64 px for `large`; `None` for an unknown host.
    let dir = fresh("urls")?;
    let mut store = IconStore::load(&dir);
    assert_eq!(store.small(ALPHA), None);
    assert_eq!(store.large(ALPHA), None);
    store.put(ALPHA, &icon(3))?;
    assert_eq!(store.small(ALPHA), Some(data_url(&icon(3).small)));
    assert_eq!(store.large(ALPHA), Some(data_url(&icon(3).large)));
    assert_eq!(store.small(BETA), None);
    Ok(())
}

#[test]
fn r8_icons_and_attempt_times_survive_a_restart() -> Res<()> {
    // R8: the time of the last attempt per host survives a restart.
    let dir = fresh("restart")?;
    let mut store = IconStore::load(&dir);
    store.put(ALPHA, &icon(4))?;
    store.record_attempt(ALPHA, 777)?;
    store.record_attempt(BETA, 888)?;
    let back = IconStore::load(&dir);
    assert_eq!(back.last_attempt(ALPHA), Some(777));
    assert_eq!(back.last_attempt(BETA), Some(888));
    assert_eq!(back.last_attempt("gamma.i2p"), None);
    assert_eq!(back.small(ALPHA), Some(data_url(&icon(4).small)));
    assert_eq!(back.large(ALPHA), Some(data_url(&icon(4).large)));
    Ok(())
}

#[test]
fn r8_the_last_attempt_is_the_newest_one() -> Res<()> {
    // R8: a new attempt replaces the old time.
    let dir = fresh("newest")?;
    let mut store = IconStore::load(&dir);
    store.record_attempt(ALPHA, 10)?;
    store.record_attempt(ALPHA, 20)?;
    assert_eq!(store.last_attempt(ALPHA), Some(20));
    assert_eq!(IconStore::load(&dir).last_attempt(ALPHA), Some(20));
    Ok(())
}

#[test]
fn r25_recording_an_attempt_leaves_the_stored_icon_alone() -> Res<()> {
    // R25: a failed fetch only records the attempt.
    let dir = fresh("keep")?;
    let mut store = IconStore::load(&dir);
    store.put(ALPHA, &icon(5))?;
    let before = fs::read(dir.join(format!("{ALPHA_STEM}.png")))?;
    store.record_attempt(ALPHA, 99)?;
    assert_eq!(fs::read(dir.join(format!("{ALPHA_STEM}.png")))?, before);
    assert_eq!(store.small(ALPHA), Some(data_url(&icon(5).small)));
    Ok(())
}

#[test]
fn r22_a_new_put_replaces_the_old_files() -> Res<()> {
    // R22: one pair of files per host.
    let dir = fresh("replace")?;
    let mut store = IconStore::load(&dir);
    store.put(ALPHA, &icon(1))?;
    store.put(ALPHA, &icon(9))?;
    assert_eq!(store.large(ALPHA), Some(data_url(&icon(9).large)));
    assert_eq!(names(&dir)?.len(), 2);
    Ok(())
}

#[test]
fn load_reads_a_missing_or_broken_folder_as_empty() -> Res<()> {
    // `load`: missing or broken files read as empty.
    let missing = fresh("missing")?.join("not-created");
    let store = IconStore::load(&missing);
    assert_eq!(store.small(ALPHA), None);
    assert_eq!(store.last_attempt(ALPHA), None);
    let dir = fresh("broken")?;
    fs::write(dir.join("attempts.json"), "{nope")?;
    let store = IconStore::load(&dir);
    assert_eq!(store.last_attempt(ALPHA), None);
    assert_eq!(store.small(ALPHA), None);
    Ok(())
}

#[test]
fn an_in_memory_store_keeps_icons_and_attempts() -> Res<()> {
    // `IconStore::default()` works without a folder.
    let mut store = IconStore::default();
    store.put(ALPHA, &icon(6))?;
    store.record_attempt(ALPHA, 5)?;
    assert_eq!(store.small(ALPHA), Some(data_url(&icon(6).small)));
    assert_eq!(store.last_attempt(ALPHA), Some(5));
    assert!(store.retain(&[])?);
    assert_eq!(store.small(ALPHA), None);
    assert_eq!(store.last_attempt(ALPHA), None);
    Ok(())
}

/// R26: no trace of beta in the store or on disk; alpha is whole.
fn assert_only_alpha_left(store: &IconStore, dir: &Path) -> Res<()> {
    assert_eq!(store.small(BETA), None);
    assert_eq!(store.large(BETA), None);
    assert_eq!(store.last_attempt(BETA), None);
    assert!(store.small(ALPHA).is_some());
    assert_eq!(store.last_attempt(ALPHA), Some(50));
    let listing = names(dir)?;
    assert!(
        !listing.iter().any(|n| n.contains(BETA_STEM)),
        "{listing:?}"
    );
    assert!(listing.contains(&format!("{ALPHA_STEM}.png")));
    assert!(listing.contains(&format!("{ALPHA_STEM}-32.png")));
    Ok(())
}

#[test]
fn r26_retain_deletes_the_files_and_attempts_of_hosts_that_are_not_live() -> Res<()> {
    // R26: alpha stays, beta goes: its files, its attempt time, its data URLs.
    let dir = fresh("retain")?;
    let mut store = IconStore::load(&dir);
    for host in [ALPHA, BETA] {
        store.put(host, &icon(1))?;
        store.record_attempt(host, 50)?;
    }
    assert!(store.retain(&live(&[ALPHA]))?, "something was deleted");
    assert_only_alpha_left(&store, &dir)?;
    assert_only_alpha_left(&IconStore::load(&dir), &dir)?;
    Ok(())
}

#[test]
fn r26_retain_returns_false_when_nothing_is_deleted() -> Res<()> {
    // `retain`: true only when something was deleted.
    let dir = fresh("noop")?;
    let mut store = IconStore::load(&dir);
    store.put(ALPHA, &icon(1))?;
    store.record_attempt(ALPHA, 1)?;
    assert!(!store.retain(&live(&[ALPHA]))?);
    assert!(store.small(ALPHA).is_some());
    let mut empty = IconStore::load(&fresh("noop-empty")?);
    assert!(!empty.retain(&[])?);
    Ok(())
}

#[test]
fn r26_retain_keeps_the_attempt_of_a_live_host_that_has_no_icon() -> Res<()> {
    // R8, R26: a live host that never got an icon keeps its attempt time.
    let dir = fresh("attempt-only")?;
    let mut store = IconStore::load(&dir);
    store.record_attempt(ALPHA, 321)?;
    store.retain(&live(&[ALPHA]))?;
    assert_eq!(store.last_attempt(ALPHA), Some(321));
    assert_eq!(IconStore::load(&dir).last_attempt(ALPHA), Some(321));
    Ok(())
}

#[test]
fn r26_retain_deletes_the_attempt_of_a_host_that_has_no_icon() -> Res<()> {
    // R26: attempt times go with the host.
    let dir = fresh("attempt-gone")?;
    let mut store = IconStore::load(&dir);
    store.record_attempt(ALPHA, 321)?;
    assert!(store.retain(&[])?);
    assert_eq!(store.last_attempt(ALPHA), None);
    let after = fs::read_to_string(dir.join("attempts.json")).unwrap_or_default();
    assert!(!after.contains(ALPHA_STEM), "{after}");
    Ok(())
}

#[test]
fn r28_retain_deletes_every_file_it_did_not_write() -> Res<()> {
    // R28: strays of any name go, also a PNG with a hash-like name of an unknown host.
    let dir = fresh("strays")?;
    let mut store = IconStore::load(&dir);
    store.put(ALPHA, &icon(1))?;
    store.record_attempt(ALPHA, 1)?;
    let ghost = "664858eb5592751cac9cdf2baf771cf69bfb056e2015b0bb9185bf406d8c4289";
    for stray in [
        "readme.txt".to_owned(),
        "favicon.png".to_owned(),
        ".DS_Store".to_owned(),
        format!("{ghost}.png"),
        format!("{ghost}-32.png"),
        format!("{ALPHA_STEM}.png.bak"),
        format!("{ALPHA_STEM}-64.png"),
    ] {
        fs::write(dir.join(stray), b"stray")?;
    }
    assert!(store.retain(&live(&[ALPHA]))?);
    let mut want = vec![
        format!("{ALPHA_STEM}.png"),
        format!("{ALPHA_STEM}-32.png"),
        "attempts.json".to_owned(),
    ];
    want.sort();
    assert_eq!(names(&dir)?, want);
    Ok(())
}

#[test]
fn r28_retain_with_no_live_host_leaves_no_icon_file() -> Res<()> {
    // R28: nothing is live, so no icon file stays.
    let dir = fresh("none-live")?;
    let mut store = IconStore::load(&dir);
    store.put(ALPHA, &icon(1))?;
    store.put(BETA, &icon(2))?;
    assert!(store.retain(&[])?);
    let left = names(&dir)?;
    assert!(
        left.iter()
            .all(|n| Path::new(n).extension().is_some_and(|e| e == "json")),
        "{left:?}"
    );
    Ok(())
}

/// R26: no trace of alpha in the store or on disk; beta is whole.
fn assert_alpha_forgotten(store: &IconStore, dir: &Path) -> Res<()> {
    assert_eq!(store.small(ALPHA), None);
    assert_eq!(store.large(ALPHA), None);
    assert_eq!(store.last_attempt(ALPHA), None);
    assert!(store.small(BETA).is_some());
    assert_eq!(store.last_attempt(BETA), Some(50));
    let listing = names(dir)?;
    assert!(
        !listing.iter().any(|n| n.contains(ALPHA_STEM)),
        "{listing:?}"
    );
    assert!(listing.contains(&format!("{BETA_STEM}.png")));
    Ok(())
}

#[test]
fn r26_forget_deletes_the_files_and_the_attempt_of_one_host() -> Res<()> {
    // R26: alpha goes, beta stays; true because something existed.
    let dir = fresh("forget")?;
    let mut store = IconStore::load(&dir);
    for host in [ALPHA, BETA] {
        store.put(host, &icon(1))?;
        store.record_attempt(host, 50)?;
    }
    assert!(store.forget(ALPHA)?);
    assert_alpha_forgotten(&store, &dir)?;
    assert_eq!(IconStore::load(&dir).last_attempt(ALPHA), None);
    assert_eq!(IconStore::load(&dir).last_attempt(BETA), Some(50));
    Ok(())
}

#[test]
fn r26_forget_returns_false_for_an_unknown_host_and_true_for_an_attempt_only_host() -> Res<()> {
    // R26: "True when one existed".
    let dir = fresh("forget-unknown")?;
    let mut store = IconStore::load(&dir);
    assert!(!store.forget(ALPHA)?);
    store.record_attempt(ALPHA, 9)?;
    assert!(store.forget(ALPHA)?);
    assert_eq!(store.last_attempt(ALPHA), None);
    assert!(!store.forget(ALPHA)?, "a second forget finds nothing");
    Ok(())
}

#[test]
fn r11_restore_attempt_puts_back_the_time_from_before() -> Res<()> {
    // R11: a new attempt was recorded; the old time comes back, also on disk.
    let dir = fresh("restore")?;
    let mut store = IconStore::load(&dir);
    store.record_attempt(ALPHA, 100)?;
    store.record_attempt(ALPHA, 900)?;
    store.restore_attempt(ALPHA, Some(100))?;
    assert_eq!(store.last_attempt(ALPHA), Some(100));
    assert_eq!(IconStore::load(&dir).last_attempt(ALPHA), Some(100));
    Ok(())
}

#[test]
fn r11_restore_attempt_with_no_earlier_time_leaves_no_attempt() -> Res<()> {
    // R11: there was no attempt before, so there is none after.
    let dir = fresh("restore-none")?;
    let mut store = IconStore::load(&dir);
    store.record_attempt(ALPHA, 900)?;
    store.restore_attempt(ALPHA, None)?;
    assert_eq!(store.last_attempt(ALPHA), None);
    assert_eq!(IconStore::load(&dir).last_attempt(ALPHA), None);
    let text = fs::read_to_string(dir.join("attempts.json")).unwrap_or_default();
    assert!(!text.contains(ALPHA_STEM), "{text}");
    Ok(())
}

#[test]
fn r11_restore_attempt_keeps_the_stored_icon() -> Res<()> {
    // R11, R25: only the time moves.
    let dir = fresh("restore-icon")?;
    let mut store = IconStore::load(&dir);
    store.put(ALPHA, &icon(2))?;
    store.record_attempt(ALPHA, 900)?;
    store.restore_attempt(ALPHA, None)?;
    assert_eq!(store.small(ALPHA), Some(data_url(&icon(2).small)));
    Ok(())
}
