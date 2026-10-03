// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Site icons, properties (R3, R4, R22 of `docs/wiki/site-icons.md`) over generated hosts.

#[path = "site_icons_support/router.rs"]
mod router;

use std::error::Error;
use std::fs;

use eepview_lib::icons::{Icon, IconStore, file_stem};
use eepview_lib::net::host::is_i2p_host;
use eepview_lib::net::icons::{FetchError, fetch, request};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestCaseError, TestRunner};
use router::{Reply, Router};

type Res<T> = Result<T, Box<dyn Error>>;

/// A runner with few cases (they open sockets or files) and no regression files.
fn runner(cases: u32) -> TestRunner {
    TestRunner::new(Config {
        cases,
        failure_persistence: None,
        ..Config::default()
    })
}

/// Hosts that are not I2P hosts, in many shapes.
fn foreign_host() -> impl Strategy<Value = String> {
    prop_oneof![
        ".{0,40}",
        "[a-z0-9.-]{1,30}\\.(com|org|net|onion|local|io)",
        "[0-9]{1,3}(\\.[0-9]{1,3}){3}",
        "[a-z]{1,10}\\.i2p[:/@?#][a-z0-9]{0,8}",
        "[a-z]{1,10}\\.i2p\\.[a-z]{2,5}",
        "[a-z]{1,6}\\.\\.i2p",
        "xn--[a-z0-9]{1,8}\\.i2p",
    ]
}

#[test]
fn r4_a_host_that_is_not_i2p_never_causes_a_request_at_the_router() -> Res<()> {
    // R4: whatever the text, no connection is opened and no request line is sent.
    let router = Router::start(|_| Reply::default())?;
    let gate = router.gatekeeper()?;
    let result = runner(64).run(&foreign_host(), |host| {
        prop_assume!(!is_i2p_host(&host));
        prop_assert_eq!(request(&host), None);
        prop_assert_eq!(fetch(&gate, &host), Err(FetchError::NotI2p));
        Ok(())
    });
    result?;
    assert_eq!(router.connections(), 0, "R4: a connection was opened");
    assert!(router.heads().is_empty(), "R4: a request was sent");
    Ok(())
}

#[test]
fn r3_the_request_for_an_i2p_host_is_exactly_the_four_lines() -> Res<()> {
    // R3: for any I2P host, the bytes are the four lines in order, and nothing else.
    runner(128)
        .run(&"[a-z0-9]{1,12}(\\.[a-z0-9]{1,8}){0,2}\\.i2p", |host| {
            prop_assume!(is_i2p_host(&host));
            let want = format!(
                "GET http://{host}/favicon.ico HTTP/1.1\r\nHost: {host}\r\nAccept: image/*\r\nConnection: close\r\n\r\n"
            );
            prop_assert_eq!(request(&host), Some(want.into_bytes()));
            Ok(())
        })?;
    Ok(())
}

#[test]
fn r22_the_file_stem_is_64_lower_case_hex_characters_for_any_host_text() -> Res<()> {
    // R22: any text, also text that is not a host.
    runner(256).run(&".{0,60}", |host| {
        let stem = file_stem(&host);
        prop_assert_eq!(stem.len(), 64);
        prop_assert!(
            stem.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')),
            "{}",
            stem
        );
        prop_assert_eq!(file_stem(&host), stem);
        Ok(())
    })?;
    Ok(())
}

#[test]
fn r22_the_file_stem_never_contains_the_host() -> Res<()> {
    // R22: a host has a dot or a letter outside a-f, so it is never a piece of a hex string.
    runner(256).run(&"[a-z0-9-]{1,20}(\\.[a-z0-9-]{1,10}){0,2}\\.i2p", |host| {
        prop_assert!(!file_stem(&host).contains(&host));
        prop_assert!(!file_stem(&host).contains(".i2p"));
        Ok(())
    })?;
    Ok(())
}

#[test]
fn r22_different_hosts_get_different_stems() -> Res<()> {
    // R22: a hash per host.
    runner(256).run(
        &("[a-z0-9]{1,12}\\.i2p", "[a-z0-9]{1,12}\\.i2p"),
        |(a, b)| {
            prop_assume!(a != b);
            prop_assert_ne!(file_stem(&a), file_stem(&b));
            Ok(())
        },
    )?;
    Ok(())
}

#[test]
fn r22_no_file_name_on_disk_contains_the_host() -> Res<()> {
    // R22, R23: put and record_attempt for any I2P host leave only hash file names.
    let root = std::env::temp_dir().join(format!("eepview-icons-props-{}", std::process::id()));
    let mut runner = runner(24);
    let counter = std::cell::Cell::new(0_u32);
    runner.run(&"[a-z]{3,12}\\.i2p", |host| {
        counter.set(counter.get() + 1);
        let dir = root.join(counter.get().to_string());
        fs::create_dir_all(&dir).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let mut store = IconStore::load(&dir);
        let icon = Icon {
            small: vec![1; 8],
            large: vec![2; 8],
        };
        store
            .put(&host, &icon)
            .map_err(|e| TestCaseError::fail(e.to_string()))?;
        store
            .record_attempt(&host, 5)
            .map_err(|e| TestCaseError::fail(e.to_string()))?;
        let stem = file_stem(&host);
        for entry in fs::read_dir(&dir).map_err(|e| TestCaseError::fail(e.to_string()))? {
            let name = entry
                .map_err(|e| TestCaseError::fail(e.to_string()))?
                .file_name();
            let name = name.to_string_lossy().into_owned();
            prop_assert!(!name.contains(".i2p") && !name.contains(&host), "{}", name);
            prop_assert!(
                name == "attempts.json" || name.starts_with(&stem),
                "{}",
                name
            );
        }
        let attempts = fs::read_to_string(dir.join("attempts.json")).unwrap_or_default();
        prop_assert!(!attempts.contains(".i2p"), "{}", attempts);
        Ok(())
    })?;
    let _ = fs::remove_dir_all(root);
    Ok(())
}
