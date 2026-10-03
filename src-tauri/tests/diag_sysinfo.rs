// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests R5.1 to R5.7 of `docs/wiki/diagnostics-and-bug-reports.md`:
//! the system information block (`eepview_lib::diag::sysinfo`). Public API only.

use eepview_lib::diag::RouterState;
use eepview_lib::diag::sysinfo::{
    self, RouterKind, SystemInfo, UptimeBucket, clean_version, linux_os, macos_version,
    windows_version,
};

fn info() -> SystemInfo {
    SystemInfo {
        version: "0.1.0".to_owned(),
        commit: "abc1234".to_owned(),
        os: "macOS 14.5".to_owned(),
        arch: "aarch64",
        engine: "WebKit 621.1.15".to_owned(),
        router_kind: RouterKind::I2pd,
        router_version: Some("2.50.0".to_owned()),
        router_state: RouterState::Ok,
        managed: true,
        js_default: true,
        tabs: 3,
        uptime: UptimeBucket::from_secs(120),
    }
}

// R5.1: `SystemInfo` has exactly these fields. The pattern has no `..`, so a new field
// (for example a locale or a screen size) breaks this test at compile time.
#[test]
fn r5_1_system_info_has_only_the_listed_fields() {
    let SystemInfo {
        version,
        commit,
        os,
        arch,
        engine,
        router_kind: _,
        router_version,
        router_state: _,
        managed,
        js_default,
        tabs,
        uptime: _,
    } = info();
    assert_eq!(version, "0.1.0");
    assert_eq!(commit, "abc1234");
    assert_eq!(os, "macOS 14.5");
    assert_eq!(arch, "aarch64");
    assert_eq!(engine, "WebKit 621.1.15");
    assert_eq!(router_version.as_deref(), Some("2.50.0"));
    assert!(managed && js_default);
    assert_eq!(tabs, 3);
}

// R5.2: the lines, in this order.
#[test]
fn r5_2_lines_follow_the_format() {
    assert_eq!(
        info().lines(),
        [
            "eepview: 0.1.0 (abc1234)",
            "OS: macOS 14.5",
            "CPU: aarch64",
            "Web engine: WebKit 621.1.15",
            "Router: i2pd 2.50.0 (managed), state ok",
            "JavaScript default: on",
            "Open tabs: 3",
            "Uptime: <10 min",
        ]
    );
}

// R5.2: no router version, an external router, JavaScript off.
#[test]
fn r5_2_router_line_without_version_and_other_values() {
    let mut other = info();
    other.router_kind = RouterKind::JavaI2p;
    other.router_version = None;
    other.router_state = RouterState::Down;
    other.managed = false;
    other.js_default = false;
    let lines = other.lines();
    assert_eq!(lines[4], "Router: Java I2P (external), state down");
    assert_eq!(lines[5], "JavaScript default: off");
}

// R5.7: there is no locale, screen, memory size or time zone.
#[test]
fn r5_7_lines_hold_no_locale_screen_memory_or_time_zone() {
    let text = info().lines().join("\n").to_lowercase();
    for word in [
        "locale",
        "language",
        "screen",
        "display",
        "memory",
        "ram",
        "time zone",
        "timezone",
    ] {
        assert!(!text.contains(word), "`{word}` in {text}");
    }
    assert_eq!(info().lines().len(), 8);
}

// R5.3: the uptime buckets and their edges.
#[test]
fn r5_3_uptime_buckets() {
    let table = [
        (0, "<1 min"),
        (59, "<1 min"),
        (60, "<10 min"),
        (599, "<10 min"),
        (600, "<1 h"),
        (3599, "<1 h"),
        (3600, ">1 h"),
        (u64::MAX, ">1 h"),
    ];
    for (secs, name) in table {
        assert_eq!(UptimeBucket::from_secs(secs).as_str(), name, "{secs}");
    }
}

// R5.4: the router kind from a text.
#[test]
fn r5_4_router_kind_from_text() {
    for text in ["i2pd", "I2PD", "i2pd 2.50.0", "I2Pd"] {
        assert_eq!(RouterKind::from_text(text).as_str(), "i2pd", "{text}");
    }
    for text in ["java", "Java I2P", "JAVA", "i2p", "I2P"] {
        assert_eq!(RouterKind::from_text(text).as_str(), "Java I2P", "{text}");
    }
    for text in ["", "nginx", "something else"] {
        assert_eq!(RouterKind::from_text(text).as_str(), "unknown", "{text}");
    }
}

// R5.5: a clean version has 1 to 5 groups of digits and is trimmed.
#[test]
fn r5_5_clean_version_accepts_digit_groups() {
    assert_eq!(clean_version("2.50.0").as_deref(), Some("2.50.0"));
    assert_eq!(clean_version("  2.50.0\n").as_deref(), Some("2.50.0"));
    assert_eq!(clean_version("1").as_deref(), Some("1"));
    assert_eq!(clean_version("1.2.3.4.5").as_deref(), Some("1.2.3.4.5"));
}

// R5.5: anything else is `None`.
#[test]
fn r5_5_clean_version_rejects_everything_else() {
    for text in [
        "",
        "   ",
        "1.2.3.4.5.6",
        "2.50.0-beta",
        "v1.2",
        "1..2",
        ".1",
        "1.",
        "1.2 extra",
        "alice-laptop",
        "https://forum.i2p",
    ] {
        assert_eq!(clean_version(text), None, "`{text}`");
    }
}

// R5.5: 32 characters at most.
#[test]
fn r5_5_clean_version_is_at_most_32_characters() {
    assert_eq!(clean_version(&"1".repeat(32)), Some("1".repeat(32)));
    assert_eq!(clean_version(&"1".repeat(33)), None);
}

// R5.6: the macOS version from `SystemVersion.plist`.
#[test]
fn r5_6_macos_version_from_plist() {
    let plist = "<?xml version=\"1.0\"?>\n<plist version=\"1.0\">\n<dict>\n\
        <key>ProductName</key>\n<string>macOS</string>\n\
        <key>ProductVersion</key>\n<string>14.5</string>\n</dict>\n</plist>\n";
    assert_eq!(macos_version(plist).as_deref(), Some("macOS 14.5"));
    assert_eq!(macos_version("<plist></plist>"), None);
    assert_eq!(macos_version(""), None);
}

// R5.6: the Linux name and version from `os-release`.
#[test]
fn r5_6_linux_os_from_os_release() {
    let text = "NAME=\"Ubuntu\"\nVERSION=\"22.04.4 LTS\"\nID=ubuntu\nVERSION_ID=\"22.04\"\n\
        HOME_URL=\"https://www.ubuntu.com/\"\n";
    assert_eq!(linux_os(text).as_deref(), Some("Linux ubuntu 22.04"));
    let quoted = "ID=\"fedora\"\nVERSION_ID=40\n";
    assert_eq!(linux_os(quoted).as_deref(), Some("Linux fedora 40"));
    assert_eq!(linux_os(""), None);
}

// R5.6: the Windows version from the `ver` output.
#[test]
fn r5_6_windows_version_from_ver_output() {
    let ver = "\nMicrosoft Windows [Version 10.0.26100.1234]\n";
    assert_eq!(windows_version(ver).as_deref(), Some("Windows 10.0.26100"));
    assert_eq!(windows_version("no version here"), None);
    assert_eq!(windows_version(""), None);
}

// R5: the module path of the public items.
#[test]
fn r5_clean_version_is_reachable_by_path() {
    assert_eq!(sysinfo::clean_version("1.0").as_deref(), Some("1.0"));
}
