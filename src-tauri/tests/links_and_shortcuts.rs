// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests for link clicks, the context menu, the keyboard shortcuts and the mouse
//! buttons. The source is `docs/wiki/links-and-shortcuts.md`: the rules L1-L13, C1-C13, P1-P5,
//! K1-K9 and B1-B3, and its "Public interface" section. Each test names its rule (`[L2]`).
//! The tests read the requirement and the public interface only, never the implementation.

use std::collections::{BTreeSet, HashMap};

use eepview_lib::context_menu::{Entry, ItemId, Target, context_menu};
use eepview_lib::core::{Core, Effect, Event, WebOp};
use eepview_lib::input::{Disposition, Modifiers, MouseButton, link_disposition};
use eepview_lib::shortcuts::{self, Action, Chord};
use eepview_lib::tabs::Place;
use eepview_lib::types::RouterStatus;
use tauri::Url;

// ---------------------------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------------------------

/// Modifiers from a spec such as `"meta+shift"`. An empty string means no modifier.
fn mods(spec: &str) -> Modifiers {
    let mut m = Modifiers::default();
    for part in spec.split('+').filter(|p| !p.is_empty()) {
        match part {
            "meta" => m.meta = true,
            "ctrl" => m.ctrl = true,
            "alt" => m.alt = true,
            "shift" => m.shift = true,
            other => panic!("bad modifier {other}"),
        }
    }
    m
}

/// The four modifier flags as one number, so a combination can be a map key.
fn bits(m: Modifiers) -> u8 {
    u8::from(m.meta) | (u8::from(m.ctrl) << 1) | (u8::from(m.alt) << 2) | (u8::from(m.shift) << 3)
}

/// Every one of the 16 modifier combinations.
fn all_modifiers() -> Vec<Modifiers> {
    (0..16u8)
        .map(|b| Modifiers {
            meta: b & 1 != 0,
            ctrl: b & 2 != 0,
            alt: b & 4 != 0,
            shift: b & 8 != 0,
        })
        .collect()
}

/// A chord from a spec such as `"meta+shift+KeyT"`: modifiers first, the key code last.
fn chord(spec: &str) -> Chord {
    let mut parts: Vec<&str> = spec.split('+').collect();
    let code = parts.pop().unwrap_or_default().to_owned();
    Chord {
        code,
        modifiers: mods(&parts.join("+")),
    }
}

fn press(code: &str, modifiers: Modifiers) -> Chord {
    Chord {
        code: code.to_owned(),
        modifiers,
    }
}

// ---------------------------------------------------------------------------------------------
// Links: link_disposition (L1-L7)
// ---------------------------------------------------------------------------------------------

const BUTTONS: [MouseButton; 6] = [
    MouseButton::None,
    MouseButton::Primary,
    MouseButton::Middle,
    MouseButton::Secondary,
    MouseButton::Back,
    MouseButton::Forward,
];

/// The rules L1-L7 as the interface section states them.
fn expected_disposition(mac: bool, m: Modifiers, button: MouseButton) -> Disposition {
    let new_tab_key = if mac { m.meta } else { m.ctrl };
    let opens_tab = match button {
        MouseButton::Middle => true,
        MouseButton::Primary | MouseButton::None => new_tab_key,
        _ => return Disposition::CurrentTab,
    };
    if m.shift {
        Disposition::NewForegroundTab
    } else if opens_tab {
        Disposition::NewBackgroundTab
    } else {
        Disposition::CurrentTab
    }
}

#[test]
fn l1_plain_primary_click_opens_in_the_same_tab_on_every_system() {
    for mac in [true, false] {
        for button in [MouseButton::Primary, MouseButton::None] {
            assert_eq!(
                link_disposition(mac, Modifiers::default(), button),
                Disposition::CurrentTab,
                "mac={mac} {button:?}"
            );
        }
    }
}

#[test]
fn l2_cmd_click_on_macos_opens_a_background_tab() {
    assert_eq!(
        link_disposition(true, mods("meta"), MouseButton::Primary),
        Disposition::NewBackgroundTab
    );
}

#[test]
fn l2_ctrl_click_on_windows_and_linux_opens_a_background_tab() {
    assert_eq!(
        link_disposition(false, mods("ctrl"), MouseButton::Primary),
        Disposition::NewBackgroundTab
    );
}

#[test]
fn l2_the_other_key_is_not_the_new_tab_key() {
    // Ctrl is not the new-tab key on macOS (it is the secondary click, L6), and Super is not the
    // new-tab key on Windows and Linux.
    assert_eq!(
        link_disposition(true, mods("ctrl"), MouseButton::Primary),
        Disposition::CurrentTab
    );
    assert_eq!(
        link_disposition(false, mods("meta"), MouseButton::Primary),
        Disposition::CurrentTab
    );
}

#[test]
fn l3_middle_click_opens_a_background_tab_on_every_system() {
    for mac in [true, false] {
        assert_eq!(
            link_disposition(mac, Modifiers::default(), MouseButton::Middle),
            Disposition::NewBackgroundTab,
            "mac={mac}"
        );
    }
}

#[test]
fn l4_new_tab_key_with_shift_opens_a_foreground_tab() {
    assert_eq!(
        link_disposition(true, mods("meta+shift"), MouseButton::Primary),
        Disposition::NewForegroundTab
    );
    assert_eq!(
        link_disposition(false, mods("ctrl+shift"), MouseButton::Primary),
        Disposition::NewForegroundTab
    );
}

#[test]
fn l4_middle_click_with_shift_opens_a_foreground_tab() {
    for mac in [true, false] {
        assert_eq!(
            link_disposition(mac, mods("shift"), MouseButton::Middle),
            Disposition::NewForegroundTab,
            "mac={mac}"
        );
    }
}

#[test]
fn l5_shift_click_alone_opens_a_foreground_tab() {
    for mac in [true, false] {
        assert_eq!(
            link_disposition(mac, mods("shift"), MouseButton::Primary),
            Disposition::NewForegroundTab,
            "mac={mac}"
        );
    }
}

#[test]
fn l6_alt_never_changes_the_result() {
    for mac in [true, false] {
        for button in BUTTONS {
            for m in all_modifiers() {
                let with_alt = Modifiers { alt: true, ..m };
                let without_alt = Modifiers { alt: false, ..m };
                assert_eq!(
                    link_disposition(mac, with_alt, button),
                    link_disposition(mac, without_alt, button),
                    "mac={mac} {button:?} {m:?}"
                );
            }
        }
    }
}

#[test]
fn l6_alt_click_acts_as_a_plain_click() {
    for mac in [true, false] {
        assert_eq!(
            link_disposition(mac, mods("alt"), MouseButton::Primary),
            Disposition::CurrentTab,
            "mac={mac}"
        );
    }
}

#[test]
fn l6_macos_ctrl_click_never_opens_a_tab() {
    // Ctrl+click on macOS is a secondary click: the shell opens the context menu and never
    // follows the link. The disposition must not be a new tab.
    assert_eq!(
        link_disposition(true, mods("ctrl"), MouseButton::Primary),
        Disposition::CurrentTab
    );
}

#[test]
fn l6_the_key_that_is_not_the_new_tab_key_changes_nothing() {
    for button in BUTTONS {
        for m in all_modifiers() {
            let mac_ctrl_on = Modifiers { ctrl: true, ..m };
            let mac_ctrl_off = Modifiers { ctrl: false, ..m };
            assert_eq!(
                link_disposition(true, mac_ctrl_on, button),
                link_disposition(true, mac_ctrl_off, button),
                "macOS ctrl {button:?} {m:?}"
            );
            let win_meta_on = Modifiers { meta: true, ..m };
            let win_meta_off = Modifiers { meta: false, ..m };
            assert_eq!(
                link_disposition(false, win_meta_on, button),
                link_disposition(false, win_meta_off, button),
                "Windows Super {button:?} {m:?}"
            );
        }
    }
}

#[test]
fn l7_a_keyboard_activation_follows_the_click_rules() {
    // Enter on a focused link reports `MouseButton::None`: Enter alone, new-tab key+Enter and
    // new-tab key+Shift+Enter.
    for mac in [true, false] {
        let key = if mac { "meta" } else { "ctrl" };
        let key_shift = format!("{key}+shift");
        assert_eq!(
            link_disposition(mac, Modifiers::default(), MouseButton::None),
            Disposition::CurrentTab
        );
        assert_eq!(
            link_disposition(mac, mods(key), MouseButton::None),
            Disposition::NewBackgroundTab
        );
        assert_eq!(
            link_disposition(mac, mods(&key_shift), MouseButton::None),
            Disposition::NewForegroundTab
        );
    }
}

#[test]
fn l1_to_l7_secondary_back_and_forward_buttons_never_follow_a_link() {
    for mac in [true, false] {
        for button in [
            MouseButton::Secondary,
            MouseButton::Back,
            MouseButton::Forward,
        ] {
            for m in all_modifiers() {
                assert_eq!(
                    link_disposition(mac, m, button),
                    Disposition::CurrentTab,
                    "mac={mac} {button:?} {m:?}"
                );
            }
        }
    }
}

#[test]
fn l1_to_l7_full_table_of_system_by_modifiers_by_button() {
    for mac in [true, false] {
        for button in BUTTONS {
            for m in all_modifiers() {
                assert_eq!(
                    link_disposition(mac, m, button),
                    expected_disposition(mac, m, button),
                    "mac={mac} {button:?} {m:?}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Links: Core::open_link (L2, L4, L8, L9, L12, L13)
// ---------------------------------------------------------------------------------------------

const SITE: &str = "http://reg.i2p/";

fn ok_status() -> RouterStatus {
    RouterStatus {
        state: "ok",
        proxy: "127.0.0.1:4444".into(),
        version: None,
        detail: None,
        paused: false,
        managed: false,
    }
}

fn core() -> Core {
    let mut c = Core::new(None, "127.0.0.1:4444", 0);
    c.router_changed(ok_status());
    c
}

fn parse(s: &str) -> Url {
    Url::parse(s).unwrap_or_else(|e| panic!("bad test url {s}: {e}"))
}

fn tab_ids(c: &Core) -> Vec<u32> {
    c.tab_infos().iter().map(|t| t.id).collect()
}

fn tab_url(c: &Core, id: u32) -> String {
    c.tab_info(id)
        .map(|t| t.url)
        .unwrap_or_else(|| panic!("no tab {id}"))
}

fn loads(fx: &[Effect]) -> Vec<(u32, String)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Web(WebOp::Load(l)) => Some((l.tab, l.url.clone())),
            _ => None,
        })
        .collect()
}

fn takes_focus(fx: &[Effect]) -> bool {
    fx.contains(&Effect::FocusToolbar)
        || fx.contains(&Effect::FocusContent)
        || fx.contains(&Effect::Emit(Event::Shortcut("focus-address")))
}

/// A core with two tabs, `[opener, other]`, and the opener active.
fn opener_and_other() -> (Core, u32, u32) {
    let mut c = core();
    let opener = c.tabs().active_id();
    let other = c
        .tab_new(None, Place::End)
        .0
        .map(|t| t.id)
        .expect("tab_new gave no tab");
    c.tab_select(opener);
    assert_eq!(c.tabs().active_id(), opener);
    (c, opener, other)
}

/// The id of the one tab that `after` has and `before` has not.
fn only_new(before: &[u32], c: &Core) -> u32 {
    let added: Vec<u32> = tab_ids(c)
        .into_iter()
        .filter(|id| !before.contains(id))
        .collect();
    assert_eq!(added.len(), 1, "expected exactly one new tab: {added:?}");
    added[0]
}

#[test]
fn l2_background_open_adds_a_tab_and_keeps_the_active_tab() {
    let (mut c, opener, _) = opener_and_other();
    let before = tab_ids(&c);
    let opener_url = tab_url(&c, opener);
    c.open_link(&parse(SITE), Disposition::NewBackgroundTab);
    let new = only_new(&before, &c);
    assert_eq!(tab_ids(&c).len(), before.len() + 1);
    assert_eq!(c.tabs().active_id(), opener, "the active tab changed");
    assert_eq!(tab_url(&c, opener), opener_url, "the current tab navigated");
    assert_eq!(tab_url(&c, new), SITE);
}

#[test]
fn l2_background_open_takes_no_focus_and_announces_the_tab() {
    let (mut c, _, _) = opener_and_other();
    let fx = c.open_link(&parse(SITE), Disposition::NewBackgroundTab);
    assert!(fx.contains(&Effect::Emit(Event::TabsChanged)), "{fx:?}");
    assert!(!takes_focus(&fx), "{fx:?}");
}

#[test]
fn l13_background_tab_loads_its_page_at_once() {
    let (mut c, _, _) = opener_and_other();
    let before = tab_ids(&c);
    let fx = c.open_link(&parse(SITE), Disposition::NewBackgroundTab);
    let new = only_new(&before, &c);
    assert_eq!(loads(&fx), vec![(new, SITE.to_owned())], "{fx:?}");
}

#[test]
fn l8_background_tab_opens_right_after_the_tab_that_opened_it() {
    let (mut c, opener, other) = opener_and_other();
    let before = tab_ids(&c);
    c.open_link(&parse(SITE), Disposition::NewBackgroundTab);
    let new = only_new(&before, &c);
    assert_eq!(tab_ids(&c), vec![opener, new, other]);
}

#[test]
fn l8_several_background_opens_keep_the_order_of_the_clicks() {
    let (mut c, opener, other) = opener_and_other();
    let mut opened = Vec::new();
    for n in 1..=3 {
        let before = tab_ids(&c);
        let url = parse(&format!("http://site{n}.i2p/"));
        c.open_link(&url, Disposition::NewBackgroundTab);
        opened.push(only_new(&before, &c));
    }
    let mut want = vec![opener];
    want.extend(&opened);
    want.push(other);
    assert_eq!(tab_ids(&c), want);
    assert_eq!(c.tabs().active_id(), opener);
}

#[test]
fn l4_foreground_open_selects_the_new_tab_and_the_current_tab_stays_put() {
    let (mut c, opener, _) = opener_and_other();
    let before = tab_ids(&c);
    let opener_url = tab_url(&c, opener);
    let fx = c.open_link(&parse(SITE), Disposition::NewForegroundTab);
    let new = only_new(&before, &c);
    assert_eq!(c.tabs().active_id(), new, "the new tab is not active");
    assert_eq!(tab_url(&c, opener), opener_url, "the current tab navigated");
    assert_eq!(loads(&fx), vec![(new, SITE.to_owned())], "{fx:?}");
}

#[test]
fn l8_foreground_tab_opens_right_after_the_tab_that_opened_it() {
    let (mut c, opener, other) = opener_and_other();
    let before = tab_ids(&c);
    c.open_link(&parse(SITE), Disposition::NewForegroundTab);
    let new = only_new(&before, &c);
    assert_eq!(tab_ids(&c), vec![opener, new, other]);
}

#[test]
fn l8_a_foreground_open_after_background_opens_continues_the_run() {
    let (mut c, opener, other) = opener_and_other();
    let before = tab_ids(&c);
    c.open_link(&parse("http://one.i2p/"), Disposition::NewBackgroundTab);
    let first = only_new(&before, &c);
    let before = tab_ids(&c);
    c.open_link(&parse("http://two.i2p/"), Disposition::NewForegroundTab);
    let second = only_new(&before, &c);
    assert_eq!(tab_ids(&c), vec![opener, first, second, other]);
}

#[test]
fn l8_selecting_a_tab_ends_the_run() {
    let (mut c, opener, other) = opener_and_other();
    let before = tab_ids(&c);
    c.open_link(&parse("http://one.i2p/"), Disposition::NewBackgroundTab);
    let first = only_new(&before, &c);
    c.tab_select(other);
    c.tab_select(opener);
    let before = tab_ids(&c);
    c.open_link(&parse("http://two.i2p/"), Disposition::NewBackgroundTab);
    let second = only_new(&before, &c);
    assert_eq!(
        tab_ids(&c),
        vec![opener, second, first, other],
        "a new run starts right after its opener"
    );
}

#[test]
fn l1_current_tab_open_navigates_the_active_tab() {
    let (mut c, opener, _) = opener_and_other();
    let before = tab_ids(&c);
    let fx = c.open_link(&parse(SITE), Disposition::CurrentTab);
    assert_eq!(tab_ids(&c), before, "no tab opens");
    assert_eq!(c.tabs().active_id(), opener);
    assert_eq!(tab_url(&c, opener), SITE);
    assert_eq!(loads(&fx), vec![(opener, SITE.to_owned())], "{fx:?}");
}

const REFUSED: [&str; 9] = [
    "http://example.com/",
    "https://example.com/page",
    "ftp://files.i2p/",
    "javascript:alert(1)",
    "file:///etc/passwd",
    "http://127.0.0.1:7657/",
    "eepview://settings",
    "http://stats.i2p.example.com/",
    "data:text/html,hi",
];

const DISPOSITIONS: [Disposition; 3] = [
    Disposition::CurrentTab,
    Disposition::NewBackgroundTab,
    Disposition::NewForegroundTab,
];

#[test]
fn l9_a_target_that_is_not_i2p_changes_no_tab_and_loads_nothing() {
    for target in REFUSED {
        for how in DISPOSITIONS {
            let (mut c, opener, _) = opener_and_other();
            let before = tab_ids(&c);
            let opener_url = tab_url(&c, opener);
            let fx = c.open_link(&parse(target), how);
            assert_eq!(tab_ids(&c), before, "{target} {how:?}: tabs changed");
            assert_eq!(c.tabs().active_id(), opener, "{target} {how:?}");
            assert_eq!(tab_url(&c, opener), opener_url, "{target} {how:?}");
            assert!(loads(&fx).is_empty(), "{target} {how:?}: {fx:?}");
        }
    }
}

#[test]
fn l9_a_refused_target_gives_one_warning_toast() {
    for target in REFUSED {
        for how in DISPOSITIONS {
            let (mut c, _, _) = opener_and_other();
            let fx = c.open_link(&parse(target), how);
            let warns = matches!(
                fx.as_slice(),
                [Effect::Emit(Event::Toast(t))] if format!("{:?}", t.kind).to_lowercase().contains("warn")
            );
            assert!(warns, "{target} {how:?}: {fx:?}");
        }
    }
}

#[test]
fn l9_a_refused_target_gives_the_same_effects_as_a_refused_new_window() {
    for target in REFUSED {
        for how in DISPOSITIONS {
            let (mut linked, _, _) = opener_and_other();
            let (mut windowed, _, _) = opener_and_other();
            let from_link = linked.open_link(&parse(target), how);
            let from_window = windowed.new_window(&parse(target));
            assert_eq!(from_link, from_window, "{target} {how:?}");
        }
    }
}

#[test]
fn l12_a_page_new_window_request_opens_a_foreground_tab() {
    let (mut c, opener, other) = opener_and_other();
    let before = tab_ids(&c);
    let fx = c.new_window(&parse(SITE));
    let new = only_new(&before, &c);
    assert_eq!(c.tabs().active_id(), new);
    assert_eq!(loads(&fx), vec![(new, SITE.to_owned())], "{fx:?}");
    assert!(tab_ids(&c).contains(&opener) && tab_ids(&c).contains(&other));
}

// ---------------------------------------------------------------------------------------------
// Keyboard: the K1 table (K1, K5, K6, K9)
// ---------------------------------------------------------------------------------------------

/// The rows of K1 for one system as `(menu id, chord)`. Quit is not in the table.
fn k1(mac: bool) -> Vec<(String, String)> {
    let key = if mac { "meta" } else { "ctrl" };
    let with = |suffix: &str| format!("{key}+{suffix}");
    let mut rows: Vec<(String, String)> = vec![
        ("new-tab".into(), with("KeyT")),
        ("new-window".into(), with("KeyN")),
        ("close-tab".into(), with("KeyW")),
        ("reopen-tab".into(), with("shift+KeyT")),
        ("focus-address".into(), with("KeyL")),
        ("reload".into(), with("KeyR")),
        ("hard-reload".into(), with("shift+KeyR")),
        ("stop".into(), "Escape".into()),
        ("back".into(), with("BracketLeft")),
        ("forward".into(), with("BracketRight")),
        ("next-tab".into(), "ctrl+Tab".into()),
        ("prev-tab".into(), "ctrl+shift+Tab".into()),
        ("open-find".into(), with("KeyF")),
        ("find-next".into(), with("KeyG")),
        ("find-prev".into(), with("shift+KeyG")),
        ("bookmark".into(), with("KeyD")),
        ("bookmarks".into(), with("shift+KeyB")),
        ("zoom-in".into(), with("Equal")),
        ("zoom-in-plus".into(), with("shift+Equal")),
        ("zoom-out".into(), with("Minus")),
        ("zoom-reset".into(), with("Digit0")),
        ("home".into(), with("shift+KeyH")),
        ("settings".into(), with("Comma")),
    ];
    for n in 1..=9 {
        rows.push((format!("tab-{n}"), with(&format!("Digit{n}"))));
    }
    rows.extend(os_rows(mac));
    rows
}

/// The rows of K1 that differ between macOS and Windows and Linux.
fn os_rows(mac: bool) -> Vec<(String, String)> {
    let pairs: &[(&str, &str)] = if mac {
        &[
            ("stop-period", "meta+Period"),
            ("next-tab-alt", "meta+shift+BracketRight"),
            ("prev-tab-alt", "meta+shift+BracketLeft"),
            ("history", "meta+KeyY"),
        ]
    } else {
        &[
            ("close-tab-f4", "ctrl+F4"),
            ("focus-address-alt", "alt+KeyD"),
            ("focus-address-f6", "F6"),
            ("reload-f5", "F5"),
            ("hard-reload-f5", "ctrl+F5"),
            ("back-alt", "alt+ArrowLeft"),
            ("forward-alt", "alt+ArrowRight"),
            ("next-tab-alt", "ctrl+PageDown"),
            ("prev-tab-alt", "ctrl+PageUp"),
            ("history", "ctrl+KeyH"),
            ("home-alt", "alt+Home"),
        ]
    };
    pairs
        .iter()
        .map(|(id, c)| ((*id).to_owned(), (*c).to_owned()))
        .collect()
}

fn action_of(id: &str) -> Action {
    shortcuts::action(id).unwrap_or_else(|| panic!("action({id:?}) is None"))
}

fn assert_rows_map_to_actions(mac: bool) {
    for (id, spec) in k1(mac) {
        let got = shortcuts::lookup(mac, &chord(&spec));
        assert!(
            got == Some(action_of(&id)),
            "mac={mac} {id} ({spec}): got {got:?}"
        );
    }
}

#[test]
fn k1_every_row_maps_to_its_action_on_macos() {
    assert_rows_map_to_actions(true);
}

#[test]
fn k1_every_row_maps_to_its_action_on_windows_and_linux() {
    assert_rows_map_to_actions(false);
}

#[test]
fn k1_extra_codes_are_checked_with_every_modifier_combination() {
    const EXTRA: [&str; 33] = [
        "KeyA",
        "KeyC",
        "KeyV",
        "KeyX",
        "KeyZ",
        "KeyQ",
        "KeyU",
        "Backspace",
        "Delete",
        "ArrowLeft",
        "ArrowRight",
        "ArrowUp",
        "ArrowDown",
        "Home",
        "End",
        "PageUp",
        "PageDown",
        "F1",
        "F4",
        "F5",
        "F6",
        "F11",
        "F12",
        "Enter",
        "Space",
        "Tab",
        "Escape",
        "Period",
        "Comma",
        "Slash",
        "Backslash",
        "Backquote",
        "Digit0",
    ];
    for mac in [true, false] {
        let mut model: HashMap<(String, u8), String> = HashMap::new();
        let mut codes: BTreeSet<String> = EXTRA.iter().map(|c| (*c).to_owned()).collect();
        for (id, spec) in k1(mac) {
            let c = chord(&spec);
            codes.insert(c.code.clone());
            let clash = model.insert((c.code, bits(c.modifiers)), id);
            assert!(clash.is_none(), "K1 gives one chord to two ids: {clash:?}");
        }
        for code in &codes {
            for m in all_modifiers() {
                let got = shortcuts::lookup(mac, &press(code, m));
                let want = model.get(&(code.clone(), bits(m))).map(|id| action_of(id));
                assert!(got == want, "mac={mac} {code} {m:?}: got {got:?}");
            }
        }
    }
}

#[test]
fn k9_ctrl_shift_t_is_reopen_never_new_tab() {
    assert!(shortcuts::lookup(false, &chord("ctrl+shift+KeyT")) == Some(action_of("reopen-tab")));
    assert!(shortcuts::lookup(true, &chord("meta+shift+KeyT")) == Some(action_of("reopen-tab")));
    assert!(
        shortcuts::lookup(false, &chord("ctrl+shift+KeyT")) != Some(Action::NewTab),
        "Ctrl+Shift+T is not New tab"
    );
}

#[test]
fn k9_cmd_t_on_windows_does_nothing_and_ctrl_t_on_macos_does_nothing() {
    assert!(shortcuts::lookup(false, &chord("meta+KeyT")).is_none());
    assert!(shortcuts::lookup(true, &chord("ctrl+KeyT")).is_none());
}

#[test]
fn k9_an_extra_or_missing_modifier_gives_no_shortcut() {
    for mac in [true, false] {
        let key = if mac { "meta" } else { "ctrl" };
        for extra in ["alt", "shift+alt", "meta+ctrl"] {
            let spec = format!("{key}+{extra}+KeyT");
            assert!(
                shortcuts::lookup(mac, &chord(&spec)).is_none(),
                "mac={mac} {spec}"
            );
        }
        assert!(
            shortcuts::lookup(mac, &chord("KeyT")).is_none(),
            "mac={mac}"
        );
        assert!(shortcuts::lookup(mac, &chord("shift+KeyT")).is_none());
    }
}

#[test]
fn k1_new_window_is_a_new_tab() {
    assert!(shortcuts::action("new-window") == Some(Action::NewTab));
    assert!(shortcuts::lookup(true, &chord("meta+KeyN")) == Some(Action::NewTab));
    assert!(shortcuts::lookup(false, &chord("ctrl+KeyN")) == Some(Action::NewTab));
}

#[test]
fn k1_the_new_menu_ids_give_the_action_of_their_row() {
    let pairs = [
        ("close-tab-f4", "close-tab"),
        ("focus-address-alt", "focus-address"),
        ("focus-address-f6", "focus-address"),
        ("reload-f5", "reload"),
        ("hard-reload-f5", "hard-reload"),
        ("stop-period", "stop"),
        ("back-alt", "back"),
        ("forward-alt", "forward"),
        ("next-tab-alt", "next-tab"),
        ("prev-tab-alt", "prev-tab"),
        ("zoom-in-plus", "zoom-in"),
        ("home-alt", "home"),
    ];
    for (alt, base) in pairs {
        assert!(
            shortcuts::action(alt) == Some(action_of(base)),
            "{alt} is not {base}"
        );
    }
}

#[test]
fn k1_ids_name_the_actions_that_the_table_says() {
    let named = [
        ("new-tab", Action::NewTab),
        ("close-tab", Action::CloseTab),
        ("reopen-tab", Action::ReopenTab),
        ("focus-address", Action::FocusAddress),
        ("reload", Action::Reload),
        ("hard-reload", Action::HardReload),
        ("stop", Action::Stop),
        ("back", Action::Back),
        ("forward", Action::Forward),
        ("next-tab", Action::NextTab),
        ("prev-tab", Action::PrevTab),
        ("find-next", Action::FindNext),
        ("find-prev", Action::FindPrev),
        ("bookmark", Action::Bookmark),
        ("bookmarks", Action::Bookmarks),
        ("history", Action::History),
        ("zoom-in", Action::ZoomIn),
        ("zoom-out", Action::ZoomOut),
        ("zoom-reset", Action::ZoomReset),
        ("home", Action::Home),
        ("settings", Action::Settings),
    ];
    for (id, want) in named {
        assert!(shortcuts::action(id) == Some(want), "{id}");
    }
    assert!(shortcuts::action("no-such-shortcut").is_none());
}

#[test]
fn k1_tab_1_to_9_are_nine_distinct_actions() {
    let actions: Vec<String> = (1..=9)
        .map(|n| format!("{:?}", action_of(&format!("tab-{n}"))))
        .collect();
    let distinct: BTreeSet<&String> = actions.iter().collect();
    assert_eq!(distinct.len(), 9, "{actions:?}");
}

/// The accelerator of one table row as a chord. `CmdOrCtrl` is Cmd on macOS and Ctrl elsewhere.
fn parse_accel(mac: bool, accel: &str) -> Chord {
    let mut parts: Vec<&str> = accel.split('+').collect();
    let code = parts.pop().unwrap_or_default().to_owned();
    let mut m = Modifiers::default();
    for part in parts {
        match part {
            "CmdOrCtrl" if mac => m.meta = true,
            "CmdOrCtrl" | "Ctrl" | "Control" => m.ctrl = true,
            "Cmd" | "Command" | "Super" => m.meta = true,
            "Alt" | "Option" => m.alt = true,
            "Shift" => m.shift = true,
            other => panic!("unknown accelerator part {other} in {accel}"),
        }
    }
    Chord { code, modifiers: m }
}

fn assert_table_matches_k1(mac: bool) {
    let table = shortcuts::table(mac);
    let ids: Vec<String> = table.iter().map(|s| s.id.to_string()).collect();
    let unique: BTreeSet<&String> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len(), "mac={mac}: ids repeat: {ids:?}");
    let want: BTreeSet<String> = k1(mac).into_iter().map(|(id, _)| id).collect();
    let got: BTreeSet<String> = ids.iter().cloned().collect();
    assert_eq!(got, want, "mac={mac}: the table is not the rows of K1");
    let chords: HashMap<String, (String, u8)> = k1(mac)
        .into_iter()
        .map(|(id, spec)| {
            let c = chord(&spec);
            (id, (c.code, bits(c.modifiers)))
        })
        .collect();
    let mut seen = BTreeSet::new();
    for row in &table {
        let c = parse_accel(mac, &row.accel.to_string());
        let key = (c.code, bits(c.modifiers));
        assert_eq!(chords.get(&row.id.to_string()), Some(&key), "{}", row.id);
        assert!(seen.insert(key), "mac={mac}: accelerators repeat");
    }
}

#[test]
fn k1_table_on_macos_has_the_rows_ids_and_accelerators_of_the_spec() {
    assert_table_matches_k1(true);
}

#[test]
fn k1_table_on_windows_and_linux_has_the_rows_ids_and_accelerators_of_the_spec() {
    assert_table_matches_k1(false);
}

#[test]
fn k1_quit_is_not_in_the_table() {
    for mac in [true, false] {
        assert!(
            shortcuts::table(mac)
                .iter()
                .all(|s| !s.id.to_string().contains("quit")),
            "mac={mac}"
        );
    }
}

#[test]
fn k1_a_row_accelerator_looks_up_to_the_action_of_the_same_row() {
    for mac in [true, false] {
        for row in shortcuts::table(mac) {
            let c = parse_accel(mac, &row.accel.to_string());
            let id = row.id.to_string();
            assert!(
                shortcuts::lookup(mac, &c) == Some(action_of(&id)),
                "mac={mac} {id}"
            );
        }
    }
}

#[test]
fn k1_no_two_actions_share_one_chord_on_one_system() {
    for mac in [true, false] {
        let mut owner: HashMap<(String, u8), String> = HashMap::new();
        for (id, spec) in k1(mac) {
            let c = chord(&spec);
            let key = (c.code, bits(c.modifiers));
            let action = format!("{:?}", action_of(&id));
            if let Some(other) = owner.insert(key.clone(), id.clone()) {
                panic!("mac={mac} {key:?} belongs to {other} and {id} ({action})");
            }
        }
    }
}

// K5: text-editing keys

fn assert_never_shortcut(mac: bool, code: &str, skip: &[Modifiers]) {
    for m in all_modifiers() {
        if skip.iter().any(|s| bits(*s) == bits(m)) {
            continue;
        }
        let got = shortcuts::lookup(mac, &press(code, m));
        assert!(got.is_none(), "mac={mac} {code} {m:?} gave {got:?}");
    }
}

#[test]
fn k5_macos_option_and_cmd_with_left_and_right_are_text_editing_keys() {
    for code in ["ArrowLeft", "ArrowRight"] {
        for spec in [
            "alt",
            "meta",
            "alt+shift",
            "meta+shift",
            "shift",
            "",
            "ctrl",
        ] {
            let got = shortcuts::lookup(true, &press(code, mods(spec)));
            assert!(got.is_none(), "macOS {spec}+{code} gave {got:?}");
        }
    }
}

#[test]
fn k5_macos_arrows_home_end_backspace_delete_are_never_shortcuts() {
    for code in [
        "ArrowLeft",
        "ArrowRight",
        "ArrowUp",
        "ArrowDown",
        "Home",
        "End",
        "Backspace",
        "Delete",
    ] {
        assert_never_shortcut(true, code, &[]);
    }
}

#[test]
fn k5_windows_ctrl_left_and_ctrl_right_are_text_editing_keys() {
    for code in ["ArrowLeft", "ArrowRight"] {
        for m in [mods("ctrl"), mods("ctrl+shift"), mods("shift"), mods("")] {
            let got = shortcuts::lookup(false, &press(code, m));
            assert!(got.is_none(), "{code} {m:?} gave {got:?}");
        }
    }
}

#[test]
fn k5_windows_home_end_backspace_delete_with_shift_or_ctrl_are_never_shortcuts() {
    assert_never_shortcut(false, "End", &[]);
    assert_never_shortcut(false, "Backspace", &[]);
    assert_never_shortcut(false, "Delete", &[]);
    assert_never_shortcut(false, "ArrowUp", &[]);
    assert_never_shortcut(false, "ArrowDown", &[]);
    // Alt+Left, Alt+Right and Alt+Home are the only shortcuts on these keys (K1).
    assert_never_shortcut(false, "ArrowLeft", &[mods("alt")]);
    assert_never_shortcut(false, "ArrowRight", &[mods("alt")]);
    assert_never_shortcut(false, "Home", &[mods("alt")]);
}

#[test]
fn k5_alt_left_and_alt_right_are_back_and_forward_on_windows_and_linux_only() {
    assert!(shortcuts::lookup(false, &chord("alt+ArrowLeft")) == Some(Action::Back));
    assert!(shortcuts::lookup(false, &chord("alt+ArrowRight")) == Some(Action::Forward));
    assert!(shortcuts::lookup(true, &chord("alt+ArrowLeft")).is_none());
    assert!(shortcuts::lookup(true, &chord("alt+ArrowRight")).is_none());
}

#[test]
fn k5_clipboard_and_undo_keys_are_never_shortcuts() {
    for mac in [true, false] {
        let key = if mac { "meta" } else { "ctrl" };
        for letter in ["KeyA", "KeyC", "KeyV", "KeyX", "KeyZ"] {
            for extra in ["", "+shift"] {
                let spec = format!("{key}{extra}+{letter}");
                let got = shortcuts::lookup(mac, &chord(&spec));
                assert!(got.is_none(), "mac={mac} {spec} gave {got:?}");
            }
        }
    }
}

#[test]
fn k6_backspace_never_goes_back_or_forward_with_any_modifier() {
    for mac in [true, false] {
        for m in all_modifiers() {
            let got = shortcuts::lookup(mac, &press("Backspace", m));
            assert!(got.is_none(), "mac={mac} Backspace {m:?} gave {got:?}");
        }
    }
}

// K4: Esc

#[test]
fn k4_escape_alone_is_stop_on_every_system() {
    for mac in [true, false] {
        assert!(
            shortcuts::lookup(mac, &chord("Escape")) == Some(Action::Stop),
            "mac={mac}"
        );
    }
}

#[test]
fn k4_escape_with_a_modifier_is_not_a_shortcut() {
    for mac in [true, false] {
        for m in all_modifiers().into_iter().filter(|m| bits(*m) != 0) {
            let got = shortcuts::lookup(mac, &press("Escape", m));
            assert!(got.is_none(), "mac={mac} Escape {m:?} gave {got:?}");
        }
    }
}

#[test]
fn k4_cmd_period_is_stop_on_macos_only() {
    assert!(shortcuts::lookup(true, &chord("meta+Period")) == Some(Action::Stop));
    assert!(shortcuts::lookup(false, &chord("meta+Period")).is_none());
    assert!(shortcuts::lookup(false, &chord("ctrl+Period")).is_none());
}

#[test]
fn k1_function_keys_exist_on_windows_and_linux_only() {
    for code in ["F5", "F6"] {
        assert!(shortcuts::lookup(false, &chord(code)).is_some(), "{code}");
        assert!(shortcuts::lookup(true, &chord(code)).is_none(), "{code}");
    }
    assert!(shortcuts::lookup(false, &chord("ctrl+F4")) == Some(Action::CloseTab));
    assert!(shortcuts::lookup(false, &chord("ctrl+F5")) == Some(Action::HardReload));
}

// P1: no forbidden item in the menu bar either

#[test]
fn p1_no_menu_bar_row_has_an_item_that_sends_data_out() {
    const FORBIDDEN: [&str; 9] = [
        "search",
        "look up",
        "translate",
        "share",
        "services",
        "download",
        "inspect",
        "writing tools",
        "autofill",
    ];
    for mac in [true, false] {
        for row in shortcuts::table(mac) {
            let label = row.label.to_string().to_lowercase();
            for word in FORBIDDEN {
                assert!(!label.contains(word), "mac={mac} {}: {label}", row.id);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Mouse (B3)
// ---------------------------------------------------------------------------------------------

#[test]
fn b3_the_mouse_back_button_goes_back() {
    assert!(shortcuts::mouse_action(MouseButton::Back) == Some(Action::Back));
}

#[test]
fn b3_the_mouse_forward_button_goes_forward() {
    assert!(shortcuts::mouse_action(MouseButton::Forward) == Some(Action::Forward));
}

#[test]
fn b3_every_other_button_gives_no_action() {
    for button in [
        MouseButton::None,
        MouseButton::Primary,
        MouseButton::Middle,
        MouseButton::Secondary,
    ] {
        let got = shortcuts::mouse_action(button);
        assert!(got.is_none(), "{button:?} gave {got:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Context menu (C2-C10, P2, P3)
// ---------------------------------------------------------------------------------------------

const I2P_LINK: &str = "http://stats.i2p/page";
const I2P_IMAGE: &str = "https://img.reg.i2p/a.png";
const CLEAR_LINK: &str = "http://example.com/page";
const CLEAR_IMAGE: &str = "http://example.com/a.png";
const I2P_PAGE: &str = "http://stats.i2p/";

fn item(id: ItemId, enabled: bool) -> Entry {
    Entry::Item { id, enabled }
}

fn target() -> Target {
    Target::default()
}

fn with_link(url: &str) -> Target {
    Target {
        link: Some(url.to_owned()),
        ..target()
    }
}

fn with_image(url: &str) -> Target {
    Target {
        image: Some(url.to_owned()),
        ..target()
    }
}

fn is_i2p(url: &str) -> bool {
    Url::parse(url).is_ok_and(|u| {
        matches!(u.scheme(), "http" | "https") && u.host_str().is_some_and(|h| h.ends_with(".i2p"))
    })
}

fn link_group(url: &str) -> Vec<Entry> {
    let open = is_i2p(url);
    vec![
        item(ItemId::OpenLinkInNewTab, open),
        item(ItemId::OpenLinkInBackgroundTab, open),
        item(ItemId::CopyLinkAddress, true),
    ]
}

fn image_group(url: &str) -> Vec<Entry> {
    let open = is_i2p(url);
    vec![
        item(ItemId::OpenImageInNewTab, open),
        item(ItemId::CopyImageAddress, true),
        item(ItemId::CopyImage, open),
    ]
}

fn editable_menu(selection: bool) -> Vec<Entry> {
    vec![
        item(ItemId::Undo, true),
        item(ItemId::Redo, true),
        Entry::Separator,
        item(ItemId::Cut, selection),
        item(ItemId::Copy, selection),
        item(ItemId::Paste, true),
        Entry::Separator,
        item(ItemId::SelectAll, true),
    ]
}

fn page_group(t: &Target) -> Vec<Entry> {
    vec![
        item(ItemId::Back, t.can_back),
        item(ItemId::Forward, t.can_forward),
        item(ItemId::Reload, true),
        Entry::Separator,
        item(ItemId::BookmarkPage, is_i2p(&t.page) && !t.bookmarked),
        item(ItemId::CopyPageAddress, true),
        Entry::Separator,
        item(ItemId::Find, true),
    ]
}

/// The menu of C3 to C8, built from the groups of the spec.
fn expected_menu(t: &Target) -> Vec<Entry> {
    if t.editable {
        return editable_menu(t.selection);
    }
    let mut groups: Vec<Vec<Entry>> = Vec::new();
    if let Some(url) = &t.link {
        groups.push(link_group(url));
    }
    if let Some(url) = &t.image {
        groups.push(image_group(url));
    }
    if t.selection {
        groups.push(vec![item(ItemId::Copy, true)]);
    }
    if groups.is_empty() {
        return page_group(t);
    }
    let mut menu = Vec::new();
    for (i, group) in groups.into_iter().enumerate() {
        if i > 0 {
            menu.push(Entry::Separator);
        }
        menu.extend(group);
    }
    menu
}

#[test]
fn c3_a_text_field_menu_is_undo_redo_cut_copy_paste_select_all() {
    let t = Target {
        editable: true,
        selection: true,
        ..target()
    };
    assert_eq!(context_menu(&t), editable_menu(true));
}

#[test]
fn c3_cut_and_copy_are_disabled_when_no_text_is_selected() {
    let t = Target {
        editable: true,
        selection: false,
        ..target()
    };
    assert_eq!(context_menu(&t), editable_menu(false));
}

#[test]
fn c3_a_link_or_an_image_under_the_pointer_adds_nothing_in_a_text_field() {
    for selection in [true, false] {
        let base = Target {
            editable: true,
            selection,
            ..target()
        };
        let linked = Target {
            link: Some(I2P_LINK.to_owned()),
            ..base.clone()
        };
        let imaged = Target {
            image: Some(I2P_IMAGE.to_owned()),
            ..base.clone()
        };
        let both = Target {
            link: Some(I2P_LINK.to_owned()),
            image: Some(I2P_IMAGE.to_owned()),
            ..base.clone()
        };
        let want = editable_menu(selection);
        assert_eq!(context_menu(&linked), want);
        assert_eq!(context_menu(&imaged), want);
        assert_eq!(context_menu(&both), want);
    }
}

#[test]
fn c4_a_link_menu_is_open_in_tab_open_in_background_tab_copy_address() {
    assert_eq!(
        context_menu(&with_link(I2P_LINK)),
        vec![
            item(ItemId::OpenLinkInNewTab, true),
            item(ItemId::OpenLinkInBackgroundTab, true),
            item(ItemId::CopyLinkAddress, true),
        ]
    );
}

#[test]
fn c5_an_image_menu_is_open_in_tab_copy_address_copy_image() {
    assert_eq!(
        context_menu(&with_image(I2P_IMAGE)),
        vec![
            item(ItemId::OpenImageInNewTab, true),
            item(ItemId::CopyImageAddress, true),
            item(ItemId::CopyImage, true),
        ]
    );
}

#[test]
fn c6_selected_text_outside_a_text_field_gives_copy_only() {
    let t = Target {
        selection: true,
        ..target()
    };
    assert_eq!(context_menu(&t), vec![item(ItemId::Copy, true)]);
}

#[test]
fn c7_an_empty_target_gives_the_page_menu() {
    let t = Target {
        page: I2P_PAGE.to_owned(),
        can_back: true,
        can_forward: true,
        ..target()
    };
    assert_eq!(
        context_menu(&t),
        vec![
            item(ItemId::Back, true),
            item(ItemId::Forward, true),
            item(ItemId::Reload, true),
            Entry::Separator,
            item(ItemId::BookmarkPage, true),
            item(ItemId::CopyPageAddress, true),
            Entry::Separator,
            item(ItemId::Find, true),
        ]
    );
}

#[test]
fn c7_back_and_forward_follow_the_history_of_the_tab() {
    for (back, forward) in [(false, false), (true, false), (false, true), (true, true)] {
        let t = Target {
            page: I2P_PAGE.to_owned(),
            can_back: back,
            can_forward: forward,
            ..target()
        };
        let menu = context_menu(&t);
        assert_eq!(menu[0], item(ItemId::Back, back));
        assert_eq!(menu[1], item(ItemId::Forward, forward));
    }
}

#[test]
fn c7_bookmark_is_enabled_only_for_an_i2p_page_with_no_bookmark() {
    let cases = [
        (I2P_PAGE, false, true),
        (I2P_PAGE, true, false),
        (CLEAR_LINK, false, false),
        ("", false, false),
    ];
    for (page, bookmarked, enabled) in cases {
        let t = Target {
            page: page.to_owned(),
            bookmarked,
            ..target()
        };
        assert!(
            context_menu(&t).contains(&item(ItemId::BookmarkPage, enabled)),
            "page={page:?} bookmarked={bookmarked}"
        );
    }
}

#[test]
fn c7_the_default_target_gives_the_page_menu_with_bookmark_disabled() {
    assert_eq!(context_menu(&target()), page_group(&target()));
}

#[test]
fn c8_link_then_image_then_selection_with_one_separator_between_groups() {
    let t = Target {
        link: Some(I2P_LINK.to_owned()),
        image: Some(I2P_IMAGE.to_owned()),
        selection: true,
        page: I2P_PAGE.to_owned(),
        ..target()
    };
    let mut want = link_group(I2P_LINK);
    want.push(Entry::Separator);
    want.extend(image_group(I2P_IMAGE));
    want.push(Entry::Separator);
    want.push(item(ItemId::Copy, true));
    assert_eq!(context_menu(&t), want);
}

#[test]
fn c8_the_page_group_shows_only_when_no_other_group_shows() {
    let page_only = [ItemId::Back, ItemId::Forward, ItemId::Reload, ItemId::Find];
    let others = [
        with_link(I2P_LINK),
        with_image(I2P_IMAGE),
        Target {
            selection: true,
            ..target()
        },
        Target {
            editable: true,
            ..target()
        },
    ];
    for t in others {
        let menu = context_menu(&t);
        for id in page_only {
            assert!(
                !menu
                    .iter()
                    .any(|e| matches!(e, Entry::Item { id: i, .. } if *i == id)),
                "{t:?} shows the page group"
            );
        }
    }
}

fn target_space() -> Vec<Target> {
    let links = [None, Some(I2P_LINK), Some(CLEAR_LINK)];
    let images = [None, Some(I2P_IMAGE), Some(CLEAR_IMAGE)];
    let pages = ["", I2P_PAGE, CLEAR_LINK];
    let mut space = Vec::new();
    for link in links {
        for image in images {
            for page in pages {
                for flags in 0..32u8 {
                    space.push(Target {
                        link: link.map(str::to_owned),
                        image: image.map(str::to_owned),
                        selection: flags & 1 != 0,
                        editable: flags & 2 != 0,
                        page: page.to_owned(),
                        can_back: flags & 4 != 0,
                        can_forward: flags & 8 != 0,
                        bookmarked: flags & 16 != 0,
                    });
                }
            }
        }
    }
    space
}

#[test]
fn c2_c3_to_c8_every_target_gives_exactly_the_menu_of_the_spec() {
    for t in target_space() {
        assert_eq!(context_menu(&t), expected_menu(&t), "{t:?}");
    }
}

#[test]
fn c8_a_menu_never_starts_or_ends_with_a_separator_or_repeats_one() {
    for t in target_space() {
        let menu = context_menu(&t);
        assert!(!menu.is_empty(), "{t:?}");
        assert_ne!(menu.first(), Some(&Entry::Separator), "{t:?}");
        assert_ne!(menu.last(), Some(&Entry::Separator), "{t:?}");
        for pair in menu.windows(2) {
            assert!(pair != [Entry::Separator, Entry::Separator], "{t:?}");
        }
    }
}

#[test]
fn c2_a_menu_never_shows_the_same_item_twice() {
    for t in target_space() {
        let ids: Vec<&'static str> = context_menu(&t)
            .iter()
            .filter_map(|e| match e {
                Entry::Item { id, .. } => Some(id.as_str()),
                Entry::Separator => None,
            })
            .collect();
        let unique: BTreeSet<&&str> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "{t:?}: {ids:?}");
    }
}

const NOT_I2P_TARGETS: [&str; 8] = [
    "http://example.com/",
    "https://example.com/a.png",
    "ftp://files.i2p/a",
    "javascript:alert(1)",
    "file:///etc/passwd",
    "http://127.0.0.1:7657/",
    "eepview://settings",
    "http://stats.i2p.example.com/",
];

#[test]
fn c9_open_items_for_a_target_that_is_not_i2p_show_disabled() {
    for url in NOT_I2P_TARGETS {
        assert_eq!(
            context_menu(&with_link(url)),
            vec![
                item(ItemId::OpenLinkInNewTab, false),
                item(ItemId::OpenLinkInBackgroundTab, false),
                item(ItemId::CopyLinkAddress, true),
            ],
            "{url}"
        );
        let menu = context_menu(&with_image(url));
        assert_eq!(menu[0], item(ItemId::OpenImageInNewTab, false), "{url}");
    }
}

#[test]
fn c9_open_items_for_http_and_https_i2p_targets_are_enabled() {
    for url in [
        "http://stats.i2p/",
        "https://stats.i2p/a?b=1",
        "http://a.b.i2p/",
    ] {
        let menu = context_menu(&with_link(url));
        assert_eq!(menu[0], item(ItemId::OpenLinkInNewTab, true), "{url}");
        assert_eq!(
            menu[1],
            item(ItemId::OpenLinkInBackgroundTab, true),
            "{url}"
        );
        let menu = context_menu(&with_image(url));
        assert_eq!(menu[0], item(ItemId::OpenImageInNewTab, true), "{url}");
    }
}

#[test]
fn c10_copy_image_is_enabled_only_for_an_i2p_image_url() {
    for url in NOT_I2P_TARGETS {
        assert!(
            context_menu(&with_image(url)).contains(&item(ItemId::CopyImage, false)),
            "{url}"
        );
    }
    assert!(context_menu(&with_image(I2P_IMAGE)).contains(&item(ItemId::CopyImage, true)));
}

#[test]
fn c10_copy_address_items_stay_enabled_for_any_target() {
    for url in NOT_I2P_TARGETS {
        assert!(context_menu(&with_link(url)).contains(&item(ItemId::CopyLinkAddress, true)));
        assert!(context_menu(&with_image(url)).contains(&item(ItemId::CopyImageAddress, true)));
    }
}

// ---------------------------------------------------------------------------------------------
// Privacy of the menus (P1-P3)
// ---------------------------------------------------------------------------------------------

/// Every context menu id with its string id and its label, as the interface section lists them.
fn spec_items() -> [(ItemId, &'static str, &'static str); 18] {
    [
        (
            ItemId::OpenLinkInNewTab,
            "open-link-new-tab",
            "Open Link in New Tab",
        ),
        (
            ItemId::OpenLinkInBackgroundTab,
            "open-link-background-tab",
            "Open Link in New Background Tab",
        ),
        (
            ItemId::CopyLinkAddress,
            "copy-link-address",
            "Copy Link Address",
        ),
        (
            ItemId::OpenImageInNewTab,
            "open-image-new-tab",
            "Open Image in New Tab",
        ),
        (
            ItemId::CopyImageAddress,
            "copy-image-address",
            "Copy Image Address",
        ),
        (ItemId::CopyImage, "copy-image", "Copy Image"),
        (ItemId::Undo, "undo", "Undo"),
        (ItemId::Redo, "redo", "Redo"),
        (ItemId::Cut, "cut", "Cut"),
        (ItemId::Copy, "copy", "Copy"),
        (ItemId::Paste, "paste", "Paste"),
        (ItemId::SelectAll, "select-all", "Select All"),
        (ItemId::Back, "back", "Back"),
        (ItemId::Forward, "forward", "Forward"),
        (ItemId::Reload, "reload", "Reload"),
        (ItemId::BookmarkPage, "bookmark-page", "Bookmark This Page"),
        (
            ItemId::CopyPageAddress,
            "copy-page-address",
            "Copy Page Address",
        ),
        (ItemId::Find, "find", "Find"),
    ]
}

#[test]
fn p2_all_has_exactly_the_18_ids_of_the_spec_once_each() {
    let got: Vec<&str> = ItemId::ALL.iter().map(|id| id.as_str()).collect();
    assert_eq!(got.len(), 18);
    let got_set: BTreeSet<&str> = got.iter().copied().collect();
    let want: BTreeSet<&str> = spec_items().iter().map(|(_, s, _)| *s).collect();
    assert_eq!(got_set, want, "ALL is not the allowed set");
}

#[test]
fn p2_every_id_has_its_string_and_its_label_of_the_spec() {
    for (id, as_str, label) in spec_items() {
        assert_eq!(id.as_str(), as_str);
        assert_eq!(id.label(), label);
    }
}

#[test]
fn p2_every_menu_for_every_target_uses_only_ids_in_all() {
    for t in target_space() {
        for entry in context_menu(&t) {
            if let Entry::Item { id, .. } = entry {
                assert!(ItemId::ALL.contains(&id), "{t:?}: {}", id.as_str());
            }
        }
    }
}

#[test]
fn p1_p2_no_label_has_a_word_of_the_forbidden_list() {
    const FORBIDDEN: [&str; 12] = [
        "search",
        "look up",
        "translate",
        "share",
        "services",
        "download",
        "save",
        "inspect",
        "window",
        "writing tools",
        "autofill",
        "speech",
    ];
    for id in ItemId::ALL {
        let label = id.label().to_lowercase();
        for word in FORBIDDEN {
            assert!(!label.contains(word), "{label:?} has {word:?}");
        }
    }
}

#[test]
fn p1_p2_a_label_that_starts_with_open_ends_with_tab() {
    for id in ItemId::ALL {
        let label = id.label();
        if label.starts_with("Open") {
            assert!(label.ends_with("Tab"), "{label:?}");
        }
    }
}

#[test]
fn p1_no_label_is_a_default_engine_or_system_item() {
    const DEFAULT_ITEMS: [&str; 12] = [
        "open link",
        "open link in new window",
        "open in new window",
        "open in default browser",
        "download linked file",
        "download image",
        "save image as",
        "inspect element",
        "look up",
        "share…",
        "services",
        "speech",
    ];
    for id in ItemId::ALL {
        let label = id.label().to_lowercase();
        assert!(!DEFAULT_ITEMS.contains(&label.as_str()), "{label:?}");
    }
}

#[test]
fn p3_no_menu_has_inspect_element() {
    for t in target_space() {
        for entry in context_menu(&t) {
            if let Entry::Item { id, .. } = entry {
                assert!(!id.label().to_lowercase().contains("inspect"), "{t:?}");
                assert!(!id.as_str().contains("inspect"), "{t:?}");
            }
        }
    }
}
