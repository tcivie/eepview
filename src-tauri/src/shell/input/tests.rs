// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests for the shell glue between the platform bridge and the core. The source
//! is `docs/wiki/links-and-shortcuts.md`. Each test names its rule (`l2_`, `k4_`, `c9_`).

use super::*;

use eepview_platform::{Button, Hit, Input, MenuEntry, Native, Reply};
use tauri::test::MockRuntime;
use tauri::{App, Url};

use crate::context_menu::{self, Entry, ItemId, PageHistory, Target};
use crate::input::{Disposition, MAC, Modifiers, MouseButton, link_disposition};
use crate::shell::testing::{bare, core};
use crate::shortcuts::Action;
use crate::tabs::Place;

type TestApp = App<MockRuntime>;

const SITE: &str = "http://reg.i2p/";
const OTHER: &str = "http://other.i2p/";
const LINK: &str = "http://a.i2p/x";
const IMAGE: &str = "http://b.i2p/y.png";
const CLEAR: &str = "http://example.com/page";

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

/// Modifiers from a spec such as `"meta+shift"`. An empty string means none.
fn mods(spec: &str) -> Modifiers {
    let mut m = Modifiers::default();
    // 43 is the plus sign; a char literal would break the lizard Rust reader.
    let plus = char::from(43u8);
    for part in spec.split(plus).filter(|p| !p.is_empty()) {
        m = match part {
            "meta" => m.with_meta(true),
            "ctrl" => m.with_ctrl(true),
            "alt" => m.with_alt(true),
            "shift" => m.with_shift(true),
            other => panic!("bad modifier {other}"),
        };
    }
    m
}

/// The new-tab key of this system, with `extra` held too.
fn new_tab_key(extra: &str) -> Modifiers {
    let key = if MAC { "meta" } else { "ctrl" };
    if extra.is_empty() {
        mods(key)
    } else {
        mods(&format!("{key}+{extra}"))
    }
}

/// Every one of the 16 modifier combinations.
fn all_modifiers() -> Vec<Modifiers> {
    (0..16u8)
        .map(|b| {
            Modifiers::default()
                .with_meta(b & 1 != 0)
                .with_ctrl(b & 2 != 0)
                .with_alt(b & 4 != 0)
                .with_shift(b & 8 != 0)
        })
        .collect()
}

/// Every `(mac, button, modifiers)` case for the given systems and buttons.
fn link_cases(macs: &[bool], buttons: &[MouseButton]) -> Vec<(bool, MouseButton, Modifiers)> {
    macs.iter()
        .flat_map(|&mac| {
            buttons
                .iter()
                .flat_map(move |&b| all_modifiers().into_iter().map(move |m| (mac, b, m)))
        })
        .collect()
}

const BUTTONS: [MouseButton; 6] = [
    MouseButton::None,
    MouseButton::Primary,
    MouseButton::Middle,
    MouseButton::Secondary,
    MouseButton::Back,
    MouseButton::Forward,
];

fn active(app: &TestApp) -> u32 {
    core(app).tabs().active_id()
}

fn count(app: &TestApp) -> usize {
    core(app).tabs().len()
}

fn ids(app: &TestApp) -> Vec<u32> {
    core(app).tab_infos().iter().map(|t| t.id).collect()
}

fn url_of(app: &TestApp, id: u32) -> String {
    core(app).tab_info(id).map(|t| t.url).expect("no such tab")
}

/// Adds a tab at the end with `url`, and returns its id.
fn add_tab(app: &TestApp, url: &str) -> u32 {
    let tab = core(app).tab_new(Some(url), Place::End).0;
    tab.map(|t| t.id).expect("tab_new gave no tab")
}

/// An app with two tabs, `[opener, other]`, and the opener active.
fn two_tabs() -> (TestApp, u32, u32) {
    let app = bare();
    let opener = active(&app);
    let other = add_tab(&app, OTHER);
    core(&app).tab_select(opener);
    assert_eq!(active(&app), opener);
    (app, opener, other)
}

/// The id of the one tab that `before` has not.
fn only_new(app: &TestApp, before: &[u32]) -> u32 {
    let added: Vec<u32> = ids(app)
        .into_iter()
        .filter(|id| !before.contains(id))
        .collect();
    assert_eq!(added.len(), 1, "expected exactly one new tab: {added:?}");
    added[0]
}

fn run(app: &TestApp, source: Source, input: Input) -> Reply {
    handle(app.handle(), source, input)
}

fn link_input(url: &str, keys: Modifiers, button: Button) -> Input {
    Input::Link {
        url: url.to_owned(),
        keys,
        button,
    }
}

fn key_input(code: &str, keys: Modifiers) -> Input {
    Input::Key {
        code: code.to_owned(),
        keys,
    }
}

fn chord_action(mac: bool, source: Source, code: &str, spec: &str) -> Option<Action> {
    key_action(mac, source, code, mods(spec))
}

fn hit(link: Option<&str>, image: Option<&str>, selection: bool, editable: bool) -> Hit {
    Hit {
        link: link.map(str::to_owned),
        image: image.map(str::to_owned),
        selection,
        editable,
    }
}

fn link_hit(url: &str) -> Hit {
    hit(Some(url), None, false, false)
}

fn is_consumed(reply: &Reply) -> bool {
    matches!(reply, Reply::Consume)
}

fn is_passed(reply: &Reply) -> bool {
    matches!(reply, Reply::Pass)
}

/// The entries of a `Menu` reply, or `None` for any other reply.
fn entries_of(reply: Reply) -> Option<Vec<MenuEntry>> {
    match reply {
        Reply::Menu(entries) => Some(entries),
        _ => None,
    }
}

fn parse(url: &str) -> Url {
    Url::parse(url).expect("bad test url")
}

fn target_with(link: Option<&str>, image: Option<&str>, page: &str) -> Target {
    Target {
        link: link.map(str::to_owned),
        image: image.map(str::to_owned),
        page: page.to_owned(),
        ..Target::default()
    }
}

/// The page facts of a tab, as `menu_target` must report them.
fn page_facts(app: &TestApp, id: u32) -> (String, PageHistory, bool) {
    let info = core(app).tab_info(id).expect("no such tab");
    let history = PageHistory {
        can_back: info.nav.can_back,
        can_forward: info.nav.can_forward,
    };
    (info.url, history, info.marks.bookmarked)
}

// ---------------------------------------------------------------------------------------------
// Source, MENU_PREFIX
// ---------------------------------------------------------------------------------------------

#[test]
fn k2_a_page_source_is_content_and_a_chrome_source_is_not() {
    assert!(Source::Tab(1).content());
    assert!(Source::Internal.content());
    assert!(Source::Console.content());
    assert!(!Source::Chrome("toolbar").content());
    assert!(!Source::Chrome("status").content());
}

#[test]
fn c2_the_menu_prefix_is_ctx() {
    assert_eq!(MENU_PREFIX, "ctx:");
}

// ---------------------------------------------------------------------------------------------
// key_action (K1, K2, K4, K5, K9)
// ---------------------------------------------------------------------------------------------

#[test]
fn k4_escape_from_a_page_is_stop() {
    for mac in [true, false] {
        for source in [Source::Tab(1), Source::Internal, Source::Console] {
            assert_eq!(
                chord_action(mac, source, "Escape", ""),
                Some(Action::Stop),
                "mac={mac} {source:?}"
            );
        }
    }
}

#[test]
fn k4_escape_from_chrome_is_never_a_shortcut() {
    for mac in [true, false] {
        for name in ["toolbar", "find", "status", "popup"] {
            assert_eq!(
                chord_action(mac, Source::Chrome(name), "Escape", ""),
                None,
                "mac={mac} {name}"
            );
        }
    }
}

#[test]
fn k2_the_new_tab_chord_works_from_every_source() {
    let sources = [
        Source::Tab(1),
        Source::Internal,
        Source::Console,
        Source::Chrome("toolbar"),
        Source::Chrome("find"),
    ];
    for source in sources {
        assert_eq!(
            chord_action(true, source, "KeyT", "meta"),
            Some(Action::NewTab),
            "{source:?}"
        );
        assert_eq!(
            chord_action(false, source, "KeyT", "ctrl"),
            Some(Action::NewTab),
            "{source:?}"
        );
    }
}

#[test]
fn k9_the_new_tab_chord_of_the_other_system_is_not_a_shortcut() {
    let from = Source::Chrome("toolbar");
    assert_eq!(chord_action(true, from, "KeyT", "ctrl"), None);
    assert_eq!(chord_action(false, from, "KeyT", "meta"), None);
}

#[test]
fn k9_extra_modifiers_make_it_another_chord() {
    let from = Source::Chrome("toolbar");
    assert_eq!(
        chord_action(false, from, "KeyT", "ctrl+shift"),
        Some(Action::ReopenTab)
    );
    assert_eq!(chord_action(false, from, "KeyT", "ctrl+alt"), None);
    assert_eq!(chord_action(false, from, "KeyT", ""), None);
}

#[test]
fn k5_alt_left_is_back_on_windows_and_linux_and_nothing_on_macos() {
    let from = Source::Chrome("toolbar");
    assert_eq!(
        chord_action(false, from, "ArrowLeft", "alt"),
        Some(Action::Back)
    );
    assert_eq!(chord_action(true, from, "ArrowLeft", "alt"), None);
}

#[test]
fn k6_backspace_is_never_a_shortcut_from_any_source() {
    for source in [Source::Tab(1), Source::Internal, Source::Chrome("toolbar")] {
        for (mac, _, m) in link_cases(&[true, false], &[MouseButton::None]) {
            assert_eq!(key_action(mac, source, "Backspace", m), None);
        }
    }
}

#[test]
fn k1_a_page_source_gives_the_same_action_as_the_shortcut_table_for_every_chord() {
    let codes = [
        "KeyT",
        "KeyW",
        "KeyL",
        "KeyR",
        "BracketLeft",
        "Digit1",
        "Tab",
        "F5",
    ];
    for code in codes {
        for (mac, _, m) in link_cases(&[true, false], &[MouseButton::None]) {
            let chord = crate::shortcuts::Chord {
                code: code.to_owned(),
                modifiers: m,
            };
            let want = crate::shortcuts::lookup(mac, &chord);
            assert_eq!(key_action(mac, Source::Tab(1), code, m), want);
            assert_eq!(key_action(mac, Source::Chrome("toolbar"), code, m), want);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// link_action, window_action (L1-L7, L12)
// ---------------------------------------------------------------------------------------------

#[test]
fn l1_a_plain_click_follows_no_disposition() {
    for mac in [true, false] {
        for button in [MouseButton::Primary, MouseButton::None] {
            assert_eq!(link_action(mac, Modifiers::default(), button), None);
        }
    }
}

#[test]
fn l2_the_new_tab_key_click_opens_a_background_tab() {
    assert_eq!(
        link_action(true, mods("meta"), MouseButton::Primary),
        Some(Disposition::NewBackgroundTab)
    );
    assert_eq!(
        link_action(false, mods("ctrl"), MouseButton::Primary),
        Some(Disposition::NewBackgroundTab)
    );
}

#[test]
fn l3_a_middle_click_opens_a_background_tab_on_every_system() {
    for mac in [true, false] {
        assert_eq!(
            link_action(mac, Modifiers::default(), MouseButton::Middle),
            Some(Disposition::NewBackgroundTab)
        );
    }
}

#[test]
fn l4_shift_with_the_new_tab_key_or_a_middle_click_opens_a_foreground_tab() {
    assert_eq!(
        link_action(true, mods("meta+shift"), MouseButton::Primary),
        Some(Disposition::NewForegroundTab)
    );
    assert_eq!(
        link_action(false, mods("ctrl+shift"), MouseButton::Primary),
        Some(Disposition::NewForegroundTab)
    );
    assert_eq!(
        link_action(MAC, mods("shift"), MouseButton::Middle),
        Some(Disposition::NewForegroundTab)
    );
}

#[test]
fn l5_shift_click_alone_opens_a_foreground_tab() {
    for mac in [true, false] {
        assert_eq!(
            link_action(mac, mods("shift"), MouseButton::Primary),
            Some(Disposition::NewForegroundTab)
        );
    }
}

#[test]
fn l6_alt_click_is_a_plain_click() {
    for mac in [true, false] {
        assert_eq!(link_action(mac, mods("alt"), MouseButton::Primary), None);
    }
}

#[test]
fn l6_macos_ctrl_click_opens_no_tab_whatever_the_other_keys() {
    for m in all_modifiers() {
        let with_ctrl = m.with_ctrl(true);
        for button in [MouseButton::Primary, MouseButton::None] {
            assert_eq!(link_action(true, with_ctrl, button), None, "{with_ctrl:?}");
        }
    }
}

#[test]
fn l7_a_keyboard_activation_follows_the_click_rules() {
    for mac in [true, false] {
        let key = if mac { "meta" } else { "ctrl" };
        let both = format!("{key}+shift");
        assert_eq!(
            link_action(mac, Modifiers::default(), MouseButton::None),
            None
        );
        assert_eq!(
            link_action(mac, mods(key), MouseButton::None),
            Some(Disposition::NewBackgroundTab)
        );
        assert_eq!(
            link_action(mac, mods(&both), MouseButton::None),
            Some(Disposition::NewForegroundTab)
        );
    }
}

#[test]
fn l1_to_l7_secondary_back_and_forward_buttons_never_open_a_tab() {
    let buttons = [
        MouseButton::Secondary,
        MouseButton::Back,
        MouseButton::Forward,
    ];
    for (mac, button, m) in link_cases(&[true, false], &buttons) {
        assert_eq!(
            link_action(mac, m, button),
            None,
            "mac={mac} {button:?} {m:?}"
        );
    }
}

#[test]
fn l1_to_l7_link_action_is_none_for_the_current_tab_and_the_disposition_otherwise() {
    for (mac, button, m) in link_cases(&[true, false], &BUTTONS) {
        let want = match link_disposition(mac, m, button) {
            Disposition::CurrentTab => None,
            other => Some(other),
        };
        assert_eq!(
            link_action(mac, m, button),
            want,
            "mac={mac} {button:?} {m:?}"
        );
    }
}

#[test]
fn l12_a_plain_new_window_request_is_the_plain_foreground_tab() {
    for mac in [true, false] {
        assert_eq!(window_action(mac, Modifiers::default()), None);
    }
}

#[test]
fn l12_a_new_window_request_with_the_new_tab_key_follows_l2_to_l4() {
    assert_eq!(
        window_action(true, mods("meta")),
        Some(Disposition::NewBackgroundTab)
    );
    assert_eq!(
        window_action(false, mods("ctrl")),
        Some(Disposition::NewBackgroundTab)
    );
    assert_eq!(
        window_action(true, mods("meta+shift")),
        Some(Disposition::NewForegroundTab)
    );
    assert_eq!(
        window_action(false, mods("shift")),
        Some(Disposition::NewForegroundTab)
    );
}

#[test]
fn l12_a_new_window_request_is_a_primary_click_for_every_modifier_combination() {
    for mac in [true, false] {
        for m in all_modifiers() {
            assert_eq!(
                window_action(mac, m),
                link_action(mac, m, MouseButton::Primary),
                "mac={mac} {m:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// menu_target (C1, C3, C12)
// ---------------------------------------------------------------------------------------------

#[test]
fn c12_a_chrome_source_outside_a_text_field_opens_no_menu() {
    let app = bare();
    let hits = [
        Hit::default(),
        hit(Some(LINK), Some(IMAGE), true, false),
        link_hit(LINK),
    ];
    for h in hits {
        for name in ["toolbar", "status", "popup"] {
            let got = menu_target(&core(&app), Source::Chrome(name), h.clone());
            assert_eq!(got, None, "{name} {h:?}");
        }
    }
}

#[test]
fn c3_a_chrome_text_field_gets_the_text_field_target_only() {
    let app = bare();
    for selection in [true, false] {
        let h = hit(Some(LINK), Some(IMAGE), selection, true);
        let want = Target {
            selection,
            editable: true,
            ..Target::default()
        };
        let got = menu_target(&core(&app), Source::Chrome("toolbar"), h);
        assert_eq!(got, Some(want), "selection={selection}");
    }
}

#[test]
fn c1_a_tab_target_holds_the_hit_and_the_facts_of_that_tab() {
    let (app, opener, other) = two_tabs();
    for id in [opener, other] {
        let (page, history, bookmarked) = page_facts(&app, id);
        assert!(!page.is_empty(), "tab {id} has no url");
        let h = hit(Some(LINK), Some(IMAGE), true, false);
        let want = Target {
            link: Some(LINK.to_owned()),
            image: Some(IMAGE.to_owned()),
            selection: true,
            editable: false,
            page,
            history,
            bookmarked,
        };
        let got = menu_target(&core(&app), Source::Tab(id), h);
        assert_eq!(got, Some(want), "tab {id}");
    }
}

#[test]
fn c1_a_tab_target_page_is_the_url_of_the_tab_it_names_not_the_active_tab() {
    let (app, opener, other) = two_tabs();
    let got = menu_target(&core(&app), Source::Tab(other), Hit::default());
    assert_eq!(got.map(|t| t.page), Some(url_of(&app, other)));
    assert_ne!(url_of(&app, opener), url_of(&app, other));
}

#[test]
fn c1_a_tab_target_in_a_text_field_keeps_the_editable_flag() {
    let (app, opener, _) = two_tabs();
    let h = hit(None, None, false, true);
    let got = menu_target(&core(&app), Source::Tab(opener), h);
    assert_eq!(got.map(|t| (t.editable, t.selection)), Some((true, false)));
}

#[test]
fn c1_an_internal_target_takes_the_facts_of_the_active_tab() {
    let (app, opener, _) = two_tabs();
    let (page, history, bookmarked) = page_facts(&app, opener);
    let h = hit(Some(LINK), None, false, false);
    let want = Target {
        link: Some(LINK.to_owned()),
        page,
        history,
        bookmarked,
        ..Target::default()
    };
    assert_eq!(menu_target(&core(&app), Source::Internal, h), Some(want));
}

#[test]
fn c1_an_internal_target_follows_the_active_tab_when_it_changes() {
    let (app, opener, other) = two_tabs();
    core(&app).tab_select(other);
    let got = menu_target(&core(&app), Source::Internal, Hit::default());
    assert_eq!(got.map(|t| t.page), Some(url_of(&app, other)));
    assert_ne!(url_of(&app, opener), url_of(&app, other));
}

#[test]
fn c1_a_console_target_with_no_console_tab_has_the_default_page_facts() {
    let app = bare();
    assert!(
        core(&app).console_tab().is_none(),
        "the bare core has a console tab"
    );
    let h = hit(Some(LINK), None, true, false);
    let want = Target {
        link: Some(LINK.to_owned()),
        selection: true,
        ..Target::default()
    };
    assert_eq!(menu_target(&core(&app), Source::Console, h), Some(want));
}

#[test]
fn c12_only_a_chrome_source_is_refused_a_menu() {
    let (app, opener, _) = two_tabs();
    for source in [Source::Tab(opener), Source::Internal, Source::Console] {
        let got = menu_target(&core(&app), source, Hit::default());
        assert!(got.is_some(), "{source:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// native_of, menu_entries, menu_item_of (C2, P2)
// ---------------------------------------------------------------------------------------------

#[test]
fn c3_the_editing_items_map_to_the_same_named_native() {
    let table = [
        (ItemId::Undo, Native::Undo),
        (ItemId::Redo, Native::Redo),
        (ItemId::Cut, Native::Cut),
        (ItemId::Copy, Native::Copy),
        (ItemId::Paste, Native::Paste),
        (ItemId::SelectAll, Native::SelectAll),
        (ItemId::CopyImage, Native::CopyImage),
    ];
    for (id, want) in table {
        assert_eq!(native_of(id), Some(want), "{}", id.as_str());
    }
}

#[test]
fn c2_every_other_item_has_no_native() {
    let native = [
        ItemId::Undo,
        ItemId::Redo,
        ItemId::Cut,
        ItemId::Copy,
        ItemId::Paste,
        ItemId::SelectAll,
        ItemId::CopyImage,
    ];
    for id in ItemId::ALL {
        if !native.contains(&id) {
            assert_eq!(native_of(id), None, "{}", id.as_str());
        }
    }
}

#[test]
fn c2_menu_entries_keeps_the_order_and_the_separators() {
    let menu = [
        Entry::Item {
            id: ItemId::OpenLinkInNewTab,
            enabled: true,
        },
        Entry::Separator,
        Entry::Item {
            id: ItemId::Copy,
            enabled: false,
        },
    ];
    let want = vec![
        MenuEntry::Item {
            id: "ctx:open-link-new-tab".to_owned(),
            label: "Open Link in New Tab".to_owned(),
            enabled: true,
            native: None,
        },
        MenuEntry::Separator,
        MenuEntry::Item {
            id: "ctx:copy".to_owned(),
            label: "Copy".to_owned(),
            enabled: false,
            native: Some(Native::Copy),
        },
    ];
    assert_eq!(menu_entries(&menu), want);
}

#[test]
fn c2_menu_entries_of_an_empty_menu_is_empty() {
    assert_eq!(menu_entries(&[]), Vec::new());
}

#[test]
fn c2_every_item_gets_the_prefixed_id_its_label_and_its_enabled_state() {
    for id in ItemId::ALL {
        for enabled in [true, false] {
            let got = menu_entries(&[Entry::Item { id, enabled }]);
            let want = vec![MenuEntry::Item {
                id: format!("ctx:{}", id.as_str()),
                label: id.label().to_owned(),
                enabled,
                native: native_of(id),
            }];
            assert_eq!(got, want, "{} enabled={enabled}", id.as_str());
        }
    }
}

#[test]
fn p2_menu_entries_of_a_real_menu_has_one_entry_per_entry() {
    let target = target_with(Some(LINK), Some(IMAGE), SITE);
    let menu = context_menu::context_menu(&target);
    assert_eq!(menu_entries(&menu).len(), menu.len());
}

#[test]
fn c2_menu_item_of_reads_the_prefixed_id_of_every_item() {
    for id in ItemId::ALL {
        let text = format!("ctx:{}", id.as_str());
        assert_eq!(menu_item_of(&text), Some(id), "{text}");
    }
}

#[test]
fn c2_menu_item_of_gives_nothing_for_a_menu_bar_id() {
    for text in ["new-tab", "close-tab", "reload", "stop", "tab-1", "zoom-in"] {
        assert_eq!(menu_item_of(text), None, "{text}");
    }
}

#[test]
fn c2_menu_item_of_gives_nothing_for_an_item_id_without_the_prefix() {
    for id in ItemId::ALL {
        assert_eq!(menu_item_of(id.as_str()), None, "{}", id.as_str());
    }
}

#[test]
fn c2_menu_item_of_gives_nothing_for_an_unknown_id() {
    for text in [
        "",
        "ctx:",
        "ctx:no-such-item",
        "ctx:new-tab",
        "ctx:ctx:copy",
        "ctx:Copy",
    ] {
        assert_eq!(menu_item_of(text), None, "{text:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// menu_act (C9, C10, C11)
// ---------------------------------------------------------------------------------------------

#[test]
fn c9_the_open_items_open_their_url_in_the_tab_the_item_names() {
    let target = target_with(Some(LINK), Some(IMAGE), SITE);
    assert_eq!(
        menu_act(ItemId::OpenLinkInNewTab, &target),
        Act::Open(parse(LINK), Disposition::NewForegroundTab)
    );
    assert_eq!(
        menu_act(ItemId::OpenLinkInBackgroundTab, &target),
        Act::Open(parse(LINK), Disposition::NewBackgroundTab)
    );
    assert_eq!(
        menu_act(ItemId::OpenImageInNewTab, &target),
        Act::Open(parse(IMAGE), Disposition::NewForegroundTab)
    );
}

#[test]
fn c9_an_open_item_with_no_url_does_nothing() {
    let target = target_with(None, None, SITE);
    for id in [
        ItemId::OpenLinkInNewTab,
        ItemId::OpenLinkInBackgroundTab,
        ItemId::OpenImageInNewTab,
    ] {
        assert_eq!(menu_act(id, &target), Act::None, "{}", id.as_str());
    }
}

#[test]
fn c9_an_open_link_item_ignores_the_image_and_an_open_image_item_ignores_the_link() {
    let only_image = target_with(None, Some(IMAGE), SITE);
    let only_link = target_with(Some(LINK), None, SITE);
    assert_eq!(menu_act(ItemId::OpenLinkInNewTab, &only_image), Act::None);
    assert_eq!(
        menu_act(ItemId::OpenLinkInBackgroundTab, &only_image),
        Act::None
    );
    assert_eq!(menu_act(ItemId::OpenImageInNewTab, &only_link), Act::None);
}

#[test]
fn c9_an_open_item_with_an_url_that_does_not_parse_does_nothing() {
    for bad in ["not a url", "", "::::", "/relative/path"] {
        let target = target_with(Some(bad), Some(bad), SITE);
        for id in [
            ItemId::OpenLinkInNewTab,
            ItemId::OpenLinkInBackgroundTab,
            ItemId::OpenImageInNewTab,
        ] {
            assert_eq!(menu_act(id, &target), Act::None, "{bad:?} {}", id.as_str());
        }
    }
}

#[test]
fn c10_the_copy_address_items_copy_their_url_as_it_is() {
    let target = target_with(Some(CLEAR), Some(IMAGE), SITE);
    assert_eq!(
        menu_act(ItemId::CopyLinkAddress, &target),
        Act::Copy(CLEAR.to_owned())
    );
    assert_eq!(
        menu_act(ItemId::CopyImageAddress, &target),
        Act::Copy(IMAGE.to_owned())
    );
    assert_eq!(
        menu_act(ItemId::CopyPageAddress, &target),
        Act::Copy(SITE.to_owned())
    );
}

#[test]
fn c10_a_copy_address_item_with_no_url_does_nothing() {
    let target = target_with(None, None, SITE);
    assert_eq!(menu_act(ItemId::CopyLinkAddress, &target), Act::None);
    assert_eq!(menu_act(ItemId::CopyImageAddress, &target), Act::None);
}

#[test]
fn c11_the_page_items_do_what_their_shortcuts_do() {
    let target = target_with(None, None, SITE);
    let table = [
        (ItemId::Back, Action::Back),
        (ItemId::Forward, Action::Forward),
        (ItemId::Reload, Action::Reload),
        (ItemId::BookmarkPage, Action::Bookmark),
        (ItemId::Find, Action::Find),
    ];
    for (id, want) in table {
        assert_eq!(
            menu_act(id, &target),
            Act::Shortcut(want),
            "{}",
            id.as_str()
        );
    }
}

#[test]
fn c3_the_editing_items_hand_their_native_command_to_the_engine() {
    let target = target_with(None, None, SITE);
    for id in ItemId::ALL {
        if let Some(native) = native_of(id) {
            assert_eq!(menu_act(id, &target), Act::Edit(native), "{}", id.as_str());
        }
    }
}

#[test]
fn c11_the_act_of_an_item_does_not_depend_on_the_other_target_fields() {
    let plain = target_with(None, None, SITE);
    let busy = Target {
        selection: true,
        editable: true,
        bookmarked: true,
        ..target_with(Some(LINK), Some(IMAGE), SITE)
    };
    for id in [ItemId::Back, ItemId::Forward, ItemId::Reload, ItemId::Find] {
        assert_eq!(menu_act(id, &plain), menu_act(id, &busy), "{}", id.as_str());
    }
}

// ---------------------------------------------------------------------------------------------
// handle: keys (K2, K4, K9)
// ---------------------------------------------------------------------------------------------

#[test]
fn k2_a_new_tab_chord_from_the_toolbar_is_consumed_and_opens_a_tab() {
    let app = bare();
    let before = count(&app);
    let reply = run(
        &app,
        Source::Chrome("toolbar"),
        key_input("KeyT", new_tab_key("")),
    );
    assert!(is_consumed(&reply));
    assert_eq!(count(&app), before + 1);
}

#[test]
fn k2_a_new_tab_chord_from_a_page_is_consumed_and_opens_a_tab() {
    let (app, opener, _) = two_tabs();
    let before = count(&app);
    let reply = run(
        &app,
        Source::Tab(opener),
        key_input("KeyT", new_tab_key("")),
    );
    assert!(is_consumed(&reply));
    assert_eq!(count(&app), before + 1);
}

#[test]
fn k8_new_window_does_what_new_tab_does() {
    let app = bare();
    let before = count(&app);
    let reply = run(
        &app,
        Source::Chrome("toolbar"),
        key_input("KeyN", new_tab_key("")),
    );
    assert!(is_consumed(&reply));
    assert_eq!(count(&app), before + 1);
}

#[test]
fn k4_escape_in_a_page_still_reaches_the_page() {
    let (app, opener, _) = two_tabs();
    for source in [Source::Tab(opener), Source::Internal, Source::Console] {
        let reply = run(&app, source, key_input("Escape", Modifiers::default()));
        assert!(is_passed(&reply), "{source:?}");
    }
}

#[test]
fn k4_escape_in_the_chrome_is_not_a_shortcut_and_passes() {
    let app = bare();
    let before = count(&app);
    let reply = run(
        &app,
        Source::Chrome("toolbar"),
        key_input("Escape", Modifiers::default()),
    );
    assert!(is_passed(&reply));
    assert_eq!(count(&app), before);
}

#[test]
fn k2_a_key_with_no_shortcut_passes_and_changes_nothing() {
    let (app, opener, _) = two_tabs();
    let before = ids(&app);
    for (code, spec) in [
        ("KeyZ", "ctrl"),
        ("KeyA", ""),
        ("Backspace", ""),
        ("ArrowLeft", "shift"),
    ] {
        let reply = run(&app, Source::Tab(opener), key_input(code, mods(spec)));
        assert!(is_passed(&reply), "{spec}+{code}");
    }
    assert_eq!(ids(&app), before);
    assert_eq!(active(&app), opener);
}

#[test]
fn k9_the_new_tab_chord_of_the_other_system_passes_and_opens_nothing() {
    let app = bare();
    let before = count(&app);
    let wrong = if MAC { mods("ctrl") } else { mods("meta") };
    let reply = run(&app, Source::Chrome("toolbar"), key_input("KeyT", wrong));
    assert!(is_passed(&reply));
    assert_eq!(count(&app), before);
}

// ---------------------------------------------------------------------------------------------
// handle: mouse (B3)
// ---------------------------------------------------------------------------------------------

#[test]
fn b3_the_back_and_forward_buttons_are_consumed() {
    let (app, opener, _) = two_tabs();
    for source in [
        Source::Tab(opener),
        Source::Internal,
        Source::Chrome("toolbar"),
    ] {
        for button in [Button::Back, Button::Forward] {
            let reply = run(&app, source, Input::Mouse(button));
            assert!(is_consumed(&reply), "{source:?} {button:?}");
        }
    }
}

#[test]
fn b3_every_other_button_passes() {
    let (app, opener, _) = two_tabs();
    for button in [
        Button::None,
        Button::Primary,
        Button::Middle,
        Button::Secondary,
    ] {
        let reply = run(&app, Source::Tab(opener), Input::Mouse(button));
        assert!(is_passed(&reply), "{button:?}");
    }
}

#[test]
fn b3_the_mouse_buttons_open_no_tab() {
    let (app, opener, _) = two_tabs();
    let before = ids(&app);
    for button in [Button::Back, Button::Forward] {
        run(&app, Source::Tab(opener), Input::Mouse(button));
    }
    assert_eq!(ids(&app), before);
}

// ---------------------------------------------------------------------------------------------
// handle: links (L1-L12)
// ---------------------------------------------------------------------------------------------

#[test]
fn l3_a_middle_click_on_an_i2p_link_opens_a_background_tab_after_the_opener() {
    let (app, opener, other) = two_tabs();
    let before = ids(&app);
    let url = url_of(&app, opener);
    let input = link_input(SITE, Modifiers::default(), Button::Middle);
    let reply = run(&app, Source::Tab(opener), input);
    assert!(is_consumed(&reply));
    let new = only_new(&app, &before);
    assert_eq!(
        ids(&app),
        vec![opener, new, other],
        "L8: right after the opener"
    );
    assert_eq!(active(&app), opener, "L2: the active tab did not change");
    assert_eq!(
        url_of(&app, opener),
        url,
        "the current tab did not navigate"
    );
}

#[test]
fn l2_the_new_tab_key_click_opens_a_background_tab_and_the_opener_stays_active() {
    let (app, opener, _) = two_tabs();
    let before = ids(&app);
    let input = link_input(SITE, new_tab_key(""), Button::Primary);
    let reply = run(&app, Source::Tab(opener), input);
    assert!(is_consumed(&reply));
    let new = only_new(&app, &before);
    assert_eq!(active(&app), opener);
    assert_eq!(url_of(&app, new), SITE);
}

#[test]
fn l2_the_current_tab_does_not_navigate_on_a_background_open() {
    let (app, opener, _) = two_tabs();
    let url = url_of(&app, opener);
    run(
        &app,
        Source::Tab(opener),
        link_input(SITE, new_tab_key(""), Button::Primary),
    );
    assert_eq!(url_of(&app, opener), url);
}

#[test]
fn l4_the_new_tab_key_with_shift_opens_a_foreground_tab() {
    let (app, opener, _) = two_tabs();
    let before = ids(&app);
    let url = url_of(&app, opener);
    let input = link_input(SITE, new_tab_key("shift"), Button::Primary);
    let reply = run(&app, Source::Tab(opener), input);
    assert!(is_consumed(&reply));
    let new = only_new(&app, &before);
    assert_eq!(active(&app), new, "the new tab is the active tab");
    assert_eq!(
        url_of(&app, opener),
        url,
        "the current tab did not navigate"
    );
}

#[test]
fn l4_a_middle_click_with_shift_opens_a_foreground_tab() {
    let (app, opener, _) = two_tabs();
    let before = ids(&app);
    let input = link_input(SITE, mods("shift"), Button::Middle);
    let reply = run(&app, Source::Tab(opener), input);
    assert!(is_consumed(&reply));
    assert_eq!(active(&app), only_new(&app, &before));
}

#[test]
fn l5_shift_click_in_a_page_is_consumed_and_opens_a_foreground_tab() {
    let (app, opener, _) = two_tabs();
    let before = ids(&app);
    let input = link_input(SITE, mods("shift"), Button::Primary);
    let reply = run(&app, Source::Tab(opener), input);
    assert!(is_consumed(&reply));
    assert_eq!(active(&app), only_new(&app, &before));
}

#[test]
fn l1_a_plain_primary_click_passes_and_opens_no_tab() {
    let (app, opener, _) = two_tabs();
    let before = ids(&app);
    let input = link_input(SITE, Modifiers::default(), Button::Primary);
    let reply = run(&app, Source::Tab(opener), input);
    assert!(is_passed(&reply));
    assert_eq!(ids(&app), before);
    assert_eq!(active(&app), opener);
}

#[test]
fn l7_a_keyboard_activation_with_no_keys_passes_and_with_the_new_tab_key_opens_a_tab() {
    let (app, opener, _) = two_tabs();
    let before = ids(&app);
    let plain = run(
        &app,
        Source::Tab(opener),
        link_input(SITE, Modifiers::default(), Button::None),
    );
    assert!(is_passed(&plain));
    assert_eq!(ids(&app), before);
    let keyed = run(
        &app,
        Source::Tab(opener),
        link_input(SITE, new_tab_key(""), Button::None),
    );
    assert!(is_consumed(&keyed));
    assert_eq!(count(&app), before.len() + 1);
}

#[test]
fn l6_an_alt_click_passes_and_opens_no_tab() {
    let (app, opener, _) = two_tabs();
    let before = ids(&app);
    let reply = run(
        &app,
        Source::Tab(opener),
        link_input(SITE, mods("alt"), Button::Primary),
    );
    assert!(is_passed(&reply));
    assert_eq!(ids(&app), before);
}

#[test]
fn l6_a_secondary_click_on_a_link_passes_and_opens_no_tab() {
    let (app, opener, _) = two_tabs();
    let before = ids(&app);
    for m in all_modifiers() {
        let reply = run(
            &app,
            Source::Tab(opener),
            link_input(SITE, m, Button::Secondary),
        );
        assert!(is_passed(&reply), "{m:?}");
    }
    assert_eq!(ids(&app), before);
}

#[test]
fn l6_a_macos_ctrl_click_opens_no_tab() {
    if !MAC {
        return;
    }
    let (app, opener, _) = two_tabs();
    let before = ids(&app);
    for spec in ["ctrl", "ctrl+shift", "ctrl+meta"] {
        let reply = run(
            &app,
            Source::Tab(opener),
            link_input(SITE, mods(spec), Button::Primary),
        );
        assert!(is_passed(&reply), "{spec}");
    }
    assert_eq!(ids(&app), before);
}

#[test]
fn l9_a_middle_click_on_a_link_that_is_not_i2p_is_consumed_and_opens_no_tab() {
    let (app, opener, _) = two_tabs();
    let before = ids(&app);
    let url = url_of(&app, opener);
    for target in [
        CLEAR,
        "https://example.com/",
        "http://127.0.0.1:7657/",
        "ftp://files.i2p/",
    ] {
        let input = link_input(target, Modifiers::default(), Button::Middle);
        let reply = run(&app, Source::Tab(opener), input);
        assert!(is_consumed(&reply), "{target}");
    }
    assert_eq!(ids(&app), before);
    assert_eq!(active(&app), opener);
    assert_eq!(url_of(&app, opener), url);
}

#[test]
fn l9_a_background_open_of_a_link_that_is_not_i2p_opens_no_tab() {
    let (app, opener, _) = two_tabs();
    let before = ids(&app);
    let input = link_input(CLEAR, new_tab_key(""), Button::Primary);
    let reply = run(&app, Source::Tab(opener), input);
    assert!(is_consumed(&reply));
    assert_eq!(ids(&app), before);
}

#[test]
fn l2_a_link_that_does_not_parse_passes_and_opens_no_tab() {
    let (app, opener, _) = two_tabs();
    let before = ids(&app);
    for bad in ["not a url", "", "::::"] {
        let input = link_input(bad, Modifiers::default(), Button::Middle);
        let reply = run(&app, Source::Tab(opener), input);
        assert!(is_passed(&reply), "{bad:?}");
    }
    assert_eq!(ids(&app), before);
}

#[test]
fn l8_two_middle_clicks_keep_the_order_of_the_clicks() {
    let (app, opener, other) = two_tabs();
    let mut run_ids = Vec::new();
    for url in ["http://one.i2p/", "http://two.i2p/"] {
        let before = ids(&app);
        run(
            &app,
            Source::Tab(opener),
            link_input(url, Modifiers::default(), Button::Middle),
        );
        run_ids.push(only_new(&app, &before));
    }
    let want = vec![opener, run_ids[0], run_ids[1], other];
    assert_eq!(ids(&app), want);
}

#[test]
fn l11_an_internal_page_opens_an_i2p_link_in_a_new_tab() {
    let (app, opener, _) = two_tabs();
    let before = count(&app);
    let input = link_input(SITE, Modifiers::default(), Button::Middle);
    let reply = run(&app, Source::Internal, input);
    assert!(is_consumed(&reply));
    assert_eq!(count(&app), before + 1);
    assert_eq!(active(&app), opener);
}

// ---------------------------------------------------------------------------------------------
// handle: menu (C1, C2, C12)
// ---------------------------------------------------------------------------------------------

/// What `handle` must give for a menu report: the entries of the model, or `Consume` on macOS.
fn assert_menu_reply(app: &TestApp, source: Source, h: Hit) {
    let target = menu_target(&core(app), source, h.clone());
    let reply = run(app, source, Input::Menu(h));
    if MAC {
        assert!(is_consumed(&reply), "{source:?}");
        return;
    }
    let want = target.map_or_else(Vec::new, |t| menu_entries(&context_menu::context_menu(&t)));
    assert_eq!(entries_of(reply), Some(want), "{source:?}");
}

#[test]
fn c2_the_menu_of_a_link_is_the_menu_of_the_model() {
    let (app, opener, _) = two_tabs();
    assert_menu_reply(&app, Source::Tab(opener), link_hit(LINK));
}

#[test]
fn c2_the_menu_of_every_hit_is_the_menu_of_the_model() {
    let (app, opener, _) = two_tabs();
    let hits = [
        Hit::default(),
        hit(Some(LINK), Some(IMAGE), true, false),
        hit(None, Some(IMAGE), false, false),
        hit(None, None, true, false),
        hit(None, None, true, true),
        hit(Some(CLEAR), Some(CLEAR), false, false),
    ];
    for h in hits {
        for source in [Source::Tab(opener), Source::Internal, Source::Console] {
            assert_menu_reply(&app, source, h.clone());
        }
    }
}

#[test]
fn c12_a_chrome_menu_outside_a_text_field_is_empty_and_a_text_field_menu_is_the_model() {
    let app = bare();
    assert_menu_reply(&app, Source::Chrome("toolbar"), link_hit(LINK));
    assert_menu_reply(&app, Source::Chrome("status"), Hit::default());
    assert_menu_reply(&app, Source::Chrome("toolbar"), hit(None, None, true, true));
}

#[test]
fn c12_off_macos_a_chrome_menu_outside_a_text_field_is_an_empty_list() {
    if MAC {
        return;
    }
    let app = bare();
    let reply = run(&app, Source::Chrome("toolbar"), Input::Menu(Hit::default()));
    assert_eq!(entries_of(reply), Some(Vec::new()));
}

#[test]
fn c1_a_menu_report_opens_no_tab() {
    let (app, opener, _) = two_tabs();
    let before = ids(&app);
    run(&app, Source::Tab(opener), Input::Menu(link_hit(LINK)));
    assert_eq!(ids(&app), before);
    assert_eq!(active(&app), opener);
}

// ---------------------------------------------------------------------------------------------
// chosen (C9, C11, L2, L9)
// ---------------------------------------------------------------------------------------------

fn pick(app: &TestApp, id: &str) {
    chosen(app.handle(), id);
}

#[test]
fn c9_open_link_in_background_tab_adds_a_tab_and_keeps_the_active_tab() {
    let (app, opener, _) = two_tabs();
    let before = ids(&app);
    run(&app, Source::Tab(opener), Input::Menu(link_hit(LINK)));
    pick(&app, "ctx:open-link-background-tab");
    let new = only_new(&app, &before);
    assert_eq!(active(&app), opener, "L2: the active tab did not change");
    assert_eq!(url_of(&app, new), LINK);
}

#[test]
fn c9_open_link_in_new_tab_adds_a_foreground_tab() {
    let (app, opener, _) = two_tabs();
    let before = ids(&app);
    run(&app, Source::Tab(opener), Input::Menu(link_hit(LINK)));
    pick(&app, "ctx:open-link-new-tab");
    let new = only_new(&app, &before);
    assert_eq!(active(&app), new, "L4: the new tab is the active tab");
    assert_eq!(url_of(&app, new), LINK);
}

#[test]
fn c9_open_image_in_new_tab_adds_a_foreground_tab_with_the_image_url() {
    let (app, opener, _) = two_tabs();
    let before = ids(&app);
    let h = hit(None, Some(IMAGE), false, false);
    run(&app, Source::Tab(opener), Input::Menu(h));
    pick(&app, "ctx:open-image-new-tab");
    let new = only_new(&app, &before);
    assert_eq!(active(&app), new);
    assert_eq!(url_of(&app, new), IMAGE);
}

#[test]
fn c9_an_open_item_for_a_target_that_is_not_i2p_opens_no_tab() {
    let (app, opener, _) = two_tabs();
    let before = ids(&app);
    run(&app, Source::Tab(opener), Input::Menu(link_hit(CLEAR)));
    for id in ["ctx:open-link-new-tab", "ctx:open-link-background-tab"] {
        pick(&app, id);
    }
    assert_eq!(ids(&app), before);
    assert_eq!(active(&app), opener);
}

#[test]
fn c9_a_new_tab_from_the_menu_opens_right_after_the_opener() {
    let (app, opener, other) = two_tabs();
    let before = ids(&app);
    run(&app, Source::Tab(opener), Input::Menu(link_hit(LINK)));
    pick(&app, "ctx:open-link-background-tab");
    let new = only_new(&app, &before);
    assert_eq!(ids(&app), vec![opener, new, other]);
}

#[test]
fn c9_the_item_acts_on_the_target_of_the_last_menu() {
    let (app, opener, _) = two_tabs();
    let before = ids(&app);
    run(&app, Source::Tab(opener), Input::Menu(link_hit(LINK)));
    run(&app, Source::Tab(opener), Input::Menu(link_hit(SITE)));
    pick(&app, "ctx:open-link-background-tab");
    let new = only_new(&app, &before);
    assert_eq!(url_of(&app, new), SITE);
}

#[test]
fn c3_a_chrome_text_field_menu_drops_the_link_so_an_open_item_does_nothing() {
    let (app, _, _) = two_tabs();
    let before = ids(&app);
    let h = hit(Some(LINK), Some(IMAGE), false, true);
    run(&app, Source::Chrome("toolbar"), Input::Menu(h));
    for id in [
        "ctx:open-link-new-tab",
        "ctx:open-link-background-tab",
        "ctx:open-image-new-tab",
    ] {
        pick(&app, id);
    }
    assert_eq!(ids(&app), before);
}

#[test]
fn c2_an_unknown_id_does_nothing() {
    let (app, opener, _) = two_tabs();
    let before = ids(&app);
    run(&app, Source::Tab(opener), Input::Menu(link_hit(LINK)));
    for id in ["ctx:no-such-item", "ctx:", "", "ctx:new-tab"] {
        pick(&app, id);
    }
    assert_eq!(ids(&app), before);
    assert_eq!(active(&app), opener);
}

#[test]
fn c2_an_id_without_the_prefix_does_nothing() {
    let (app, opener, _) = two_tabs();
    let before = ids(&app);
    run(&app, Source::Tab(opener), Input::Menu(link_hit(LINK)));
    pick(&app, "open-link-background-tab");
    pick(&app, "open-link-new-tab");
    assert_eq!(ids(&app), before);
}

#[test]
fn c2_a_choice_with_no_menu_before_does_nothing() {
    let (app, opener, _) = two_tabs();
    let before = ids(&app);
    for id in [
        "ctx:open-link-new-tab",
        "ctx:open-link-background-tab",
        "ctx:open-image-new-tab",
    ] {
        pick(&app, id);
    }
    assert_eq!(ids(&app), before);
    assert_eq!(active(&app), opener);
}

#[test]
fn c9_an_open_item_for_an_image_hit_with_no_link_opens_only_the_image_item() {
    let (app, opener, _) = two_tabs();
    let before = count(&app);
    let h = hit(None, Some(IMAGE), false, false);
    run(&app, Source::Tab(opener), Input::Menu(h));
    pick(&app, "ctx:open-link-new-tab");
    assert_eq!(count(&app), before, "there is no link to open");
}
