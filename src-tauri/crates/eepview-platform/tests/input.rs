// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests for the pure API of the platform bridge: the keys, the buttons, the wire
//! messages of the macOS private-world script and the key codes. The source is
//! `docs/wiki/links-and-shortcuts.md`: K1/K2 (keys), L2-L7 (links), C1/C2 (menus) and B3 (mouse).
//! The tests read the requirement and the public interface only, never the implementation.

use eepview_platform::{
    Button, HOVER_SCRIPT, Hit, INPUT_CHANNEL, Input, Keys, code_of_char, code_of_virtual_key,
    dom_button, input_script, keys_of, parse_message,
};

/// The keys named by `held`: `m` meta, `c` ctrl, `a` alt, `s` shift. Built with the `with_` methods.
fn keys(held: &str) -> Keys {
    let has = |letter: &str| held.chars().any(|c| c.to_string() == letter);
    Keys::default()
        .with_meta(has("m"))
        .with_ctrl(has("c"))
        .with_alt(has("a"))
        .with_shift(has("s"))
}

/// A `link` message from its parts.
fn link_msg(button: &str, keys: &str, url: &str) -> String {
    format!("link\t{button}\t{keys}\t{url}")
}

/// The code of the first character of `text`.
fn code(text: &str) -> Option<String> {
    text.chars().next().and_then(code_of_char)
}

// ---------------------------------------------------------------------------------------------
// Keys (K1, K2, L2-L7)
// ---------------------------------------------------------------------------------------------

#[test]
fn k1_the_default_keys_hold_nothing() {
    let none = Keys::default();
    assert!(!none.meta() && !none.ctrl() && !none.alt() && !none.shift());
}

#[test]
fn k1_each_with_method_sets_only_its_own_key() {
    assert!(keys("m").meta());
    assert!(keys("c").ctrl());
    assert!(keys("a").alt());
    assert!(keys("s").shift());
    let only_meta = Keys::default().with_meta(true);
    assert!(!only_meta.ctrl() && !only_meta.alt() && !only_meta.shift());
    let only_shift = Keys::default().with_shift(true);
    assert!(!only_shift.meta() && !only_shift.ctrl() && !only_shift.alt());
}

#[test]
fn k1_a_with_method_can_clear_a_key() {
    let all = keys("mcas");
    assert_eq!(all.with_ctrl(false), keys("mas"));
    assert_eq!(all.with_alt(false), keys("mcs"));
}

#[test]
fn k1_keys_compare_by_the_held_keys() {
    assert_eq!(keys("ma"), keys("ma"));
    assert_ne!(keys("ma"), keys("ms"));
}

#[test]
fn k1_keys_of_the_empty_text_is_no_key() {
    assert_eq!(keys_of(""), Keys::default());
}

#[test]
fn k1_keys_of_reads_each_letter() {
    assert_eq!(keys_of("m"), keys("m"));
    assert_eq!(keys_of("c"), keys("c"));
    assert_eq!(keys_of("a"), keys("a"));
    assert_eq!(keys_of("s"), keys("s"));
}

#[test]
fn k1_keys_of_reads_the_letters_in_any_order() {
    for text in ["msca", "asmc", "cmsa", "sacm", "mcas"] {
        assert_eq!(keys_of(text), keys("mcas"), "{text}");
    }
    assert_eq!(keys_of("sm"), keys_of("ms"));
    assert_eq!(keys_of("sm"), keys("ms"));
    assert_eq!(keys_of("ca"), keys("ca"));
}

#[test]
fn k1_keys_of_ignores_the_other_letters() {
    assert_eq!(keys_of("xyz"), Keys::default());
    assert_eq!(keys_of("xmz"), keys("m"));
    assert_eq!(keys_of("q-s9"), keys("s"));
}

#[test]
fn k1_keys_of_a_repeated_letter_holds_the_key_once() {
    assert_eq!(keys_of("mm"), keys("m"));
    assert_eq!(keys_of("ssc"), keys("cs"));
}

// ---------------------------------------------------------------------------------------------
// Buttons (L2-L7, B3)
// ---------------------------------------------------------------------------------------------

#[test]
fn l7_dom_button_none_is_a_keyboard_activation() {
    assert_eq!(dom_button("none"), Some(Button::None));
}

#[test]
fn l2_dom_button_numbers_are_the_mouse_buttons() {
    let table = [
        ("0", Button::Primary),
        ("1", Button::Middle),
        ("2", Button::Secondary),
        ("3", Button::Back),
        ("4", Button::Forward),
    ];
    for (text, want) in table {
        assert_eq!(dom_button(text), Some(want), "{text}");
    }
}

#[test]
fn l2_dom_button_gives_no_button_for_anything_else() {
    for text in ["", "5", "9", "-1", "10", "x", "primary", " 0", "0 "] {
        assert_eq!(dom_button(text), None, "{text:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// The wire messages: links (L2-L7)
// ---------------------------------------------------------------------------------------------

#[test]
fn l2_a_link_message_gives_the_url_the_keys_and_the_button() {
    let got = parse_message(&link_msg("0", "m", "http://stats.i2p/page"));
    let want = Input::Link {
        url: "http://stats.i2p/page".to_owned(),
        keys: keys("m"),
        button: Button::Primary,
    };
    assert_eq!(got, Some(want));
}

#[test]
fn l3_a_middle_click_link_message_has_the_middle_button() {
    let got = parse_message(&link_msg("1", "", "http://stats.i2p/"));
    let want = Input::Link {
        url: "http://stats.i2p/".to_owned(),
        keys: Keys::default(),
        button: Button::Middle,
    };
    assert_eq!(got, Some(want));
}

#[test]
fn l4_a_link_message_carries_every_key_letter() {
    let got = parse_message(&link_msg("0", "msca", "http://a.i2p/"));
    let want = Input::Link {
        url: "http://a.i2p/".to_owned(),
        keys: keys("mcas"),
        button: Button::Primary,
    };
    assert_eq!(got, Some(want));
}

#[test]
fn l7_a_keyboard_activation_link_message_has_the_none_button() {
    let got = parse_message(&link_msg("none", "c", "http://a.i2p/"));
    let want = Input::Link {
        url: "http://a.i2p/".to_owned(),
        keys: keys("c"),
        button: Button::None,
    };
    assert_eq!(got, Some(want));
}

#[test]
fn l2_every_button_number_reaches_the_link_message() {
    let table = [
        ("0", Button::Primary),
        ("1", Button::Middle),
        ("2", Button::Secondary),
        ("3", Button::Back),
        ("4", Button::Forward),
        ("none", Button::None),
    ];
    for (text, want) in table {
        let got = parse_message(&link_msg(text, "", "http://a.i2p/"));
        let button = match got {
            Some(Input::Link { button, .. }) => Some(button),
            _ => None,
        };
        assert_eq!(button, Some(want), "{text}");
    }
}

#[test]
fn l2_a_link_message_with_an_empty_url_gives_nothing() {
    assert_eq!(parse_message(&link_msg("0", "m", "")), None);
    assert_eq!(parse_message(&link_msg("1", "", "")), None);
}

#[test]
fn l2_a_link_message_with_the_wrong_field_count_gives_nothing() {
    for text in [
        "link",
        "link\t0",
        "link\t0\tm",
        "link\t0\tm\thttp://a.i2p/\textra",
    ] {
        assert_eq!(parse_message(text), None, "{text:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// The wire messages: keys (K2, K4)
// ---------------------------------------------------------------------------------------------

#[test]
fn k2_a_key_message_gives_the_code_and_the_keys() {
    let got = parse_message("key\tKeyT\tm");
    let want = Input::Key {
        code: "KeyT".to_owned(),
        keys: keys("m"),
    };
    assert_eq!(got, Some(want));
}

#[test]
fn k4_an_escape_key_message_with_no_keys_gives_escape_and_no_keys() {
    let got = parse_message("key\tEscape\t");
    let want = Input::Key {
        code: "Escape".to_owned(),
        keys: Keys::default(),
    };
    assert_eq!(got, Some(want));
}

#[test]
fn k2_a_key_message_carries_every_key_letter() {
    let got = parse_message("key\tBracketLeft\tsmca");
    let want = Input::Key {
        code: "BracketLeft".to_owned(),
        keys: keys("mcas"),
    };
    assert_eq!(got, Some(want));
}

#[test]
fn k2_a_key_message_with_an_empty_code_gives_nothing() {
    assert_eq!(parse_message("key\t\tm"), None);
    assert_eq!(parse_message("key\t\t"), None);
}

#[test]
fn k2_a_key_message_with_the_wrong_field_count_gives_nothing() {
    for text in ["key", "key\tKeyT", "key\tKeyT\tm\textra"] {
        assert_eq!(parse_message(text), None, "{text:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// The wire messages: mouse (B3)
// ---------------------------------------------------------------------------------------------

#[test]
fn b3_mouse_3_is_the_back_button() {
    assert_eq!(parse_message("mouse\t3"), Some(Input::Mouse(Button::Back)));
}

#[test]
fn b3_mouse_4_is_the_forward_button() {
    assert_eq!(
        parse_message("mouse\t4"),
        Some(Input::Mouse(Button::Forward))
    );
}

#[test]
fn b3_every_other_mouse_button_gives_nothing() {
    for text in [
        "mouse\t0", "mouse\t1", "mouse\t2", "mouse\t5", "mouse\t", "mouse\tx",
    ] {
        assert_eq!(parse_message(text), None, "{text:?}");
    }
}

#[test]
fn b3_a_mouse_message_with_the_wrong_field_count_gives_nothing() {
    for text in ["mouse", "mouse\t3\t4", "mouse\t4\tm"] {
        assert_eq!(parse_message(text), None, "{text:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// The wire messages: menu (C1, C2)
// ---------------------------------------------------------------------------------------------

/// The `Hit` of a `menu` message, or `None` when the message is not a menu report.
fn hit_of(text: &str) -> Option<Hit> {
    match parse_message(text) {
        Some(Input::Menu(hit)) => Some(hit),
        _ => None,
    }
}

#[test]
fn c1_a_menu_message_gives_the_flags_the_link_and_the_image() {
    let got = hit_of("menu\tse\thttp://a.i2p/x\thttp://b.i2p/y.png");
    let want = Hit {
        link: Some("http://a.i2p/x".to_owned()),
        image: Some("http://b.i2p/y.png".to_owned()),
        selection: true,
        editable: true,
    };
    assert_eq!(got, Some(want));
}

#[test]
fn c1_a_menu_message_with_no_flags_is_not_a_selection_and_not_editable() {
    let got = hit_of("menu\t\thttp://a.i2p/x\thttp://b.i2p/y.png");
    assert_eq!(got.map(|h| (h.selection, h.editable)), Some((false, false)));
}

#[test]
fn c1_flag_s_is_the_selection_and_flag_e_is_editable() {
    let selection = hit_of("menu\ts\t\t");
    let editable = hit_of("menu\te\t\t");
    assert_eq!(
        selection.map(|h| (h.selection, h.editable)),
        Some((true, false))
    );
    assert_eq!(
        editable.map(|h| (h.selection, h.editable)),
        Some((false, true))
    );
}

#[test]
fn c1_the_flags_come_in_any_order() {
    let a = hit_of("menu\tse\t\t");
    let b = hit_of("menu\tes\t\t");
    assert_eq!(
        a.as_ref().map(|h| (h.selection, h.editable)),
        Some((true, true))
    );
    assert_eq!(a, b);
}

#[test]
fn c2_an_empty_link_field_is_no_link_and_an_empty_image_field_is_no_image() {
    let none = hit_of("menu\t\t\t");
    assert_eq!(none, Some(Hit::default()));
    let only_image = hit_of("menu\t\t\thttp://b.i2p/y.png");
    assert_eq!(only_image.as_ref().and_then(|h| h.link.clone()), None);
    assert_eq!(
        only_image.and_then(|h| h.image),
        Some("http://b.i2p/y.png".to_owned())
    );
    let only_link = hit_of("menu\t\thttp://a.i2p/x\t");
    assert_eq!(only_link.as_ref().and_then(|h| h.image.clone()), None);
    assert_eq!(
        only_link.and_then(|h| h.link),
        Some("http://a.i2p/x".to_owned())
    );
}

#[test]
fn c2_a_menu_message_with_the_wrong_field_count_gives_nothing() {
    for text in [
        "menu",
        "menu\ts",
        "menu\ts\thttp://a.i2p/",
        "menu\ts\t\t\textra",
    ] {
        assert_eq!(parse_message(text), None, "{text:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// The wire messages: anything else
// ---------------------------------------------------------------------------------------------

#[test]
fn l2_an_unknown_kind_or_an_empty_message_gives_nothing() {
    for text in [
        "",
        "\t",
        "scroll\t1",
        "x",
        "hello world",
        "links\t0\tm\thttp://a.i2p/",
    ] {
        assert_eq!(parse_message(text), None, "{text:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// The macOS private-world script (C1, C2, L2-L7, K4, B3)
// ---------------------------------------------------------------------------------------------

#[test]
fn c1_the_channel_is_the_message_handler_name() {
    assert_eq!(INPUT_CHANNEL, "eepviewInput");
}

#[test]
fn c1_the_script_always_cancels_the_engine_menu_and_posts_through_the_channel() {
    for content in [true, false] {
        let script = input_script(content);
        assert!(script.contains("contextmenu"), "content={content}");
        assert!(script.contains("preventDefault"), "content={content}");
        assert!(
            script.contains("window.webkit.messageHandlers.eepviewInput"),
            "content={content}"
        );
    }
}

#[test]
fn c1_the_script_posts_through_the_name_of_the_constant() {
    let handler = format!("window.webkit.messageHandlers.{INPUT_CHANNEL}");
    for content in [true, false] {
        assert!(
            input_script(content).contains(&handler),
            "content={content}"
        );
    }
}

#[test]
fn l11_the_script_guards_itself_so_it_runs_once_per_document() {
    for content in [true, false] {
        let script = input_script(content);
        assert!(
            script.contains("window.__eepviewInput"),
            "content={content}"
        );
    }
}

#[test]
fn l2_the_content_script_listens_for_clicks_and_aux_clicks_on_links() {
    let script = input_script(true);
    assert!(script.contains("auxclick"));
    let quoted_click = script.contains("\"click\"") || script.contains("\u{27}click\u{27}");
    assert!(quoted_click, "no click listener");
}

#[test]
fn k4_the_content_script_reports_the_escape_key() {
    let script = input_script(true);
    assert!(script.contains("keydown"));
    assert!(script.contains("Escape"));
}

#[test]
fn b3_the_content_script_reports_the_back_and_forward_buttons() {
    assert!(input_script(true).contains("mouseup"));
}

#[test]
fn c12_the_chrome_script_has_only_the_context_menu_listener() {
    let script = input_script(false);
    assert!(script.contains("contextmenu"));
    assert!(!script.contains("auxclick"));
    assert!(!script.contains("keydown"));
    assert!(!script.contains("mouseup"));
    assert!(!script.contains("Escape"));
    let quoted_click = script.contains("\"click\"") || script.contains("\u{27}click\u{27}");
    assert!(!quoted_click, "a chrome page reports no link click");
}

#[test]
fn c2_the_content_script_is_longer_than_the_chrome_script() {
    assert!(input_script(true).len() > input_script(false).len());
    assert_ne!(input_script(true), input_script(false));
}

/// Every `addEventListener` call of `script`: the event name and the handler text. The
/// handler text starts at the handler's parameters: of the inline arrow function, or of the
/// `const <name> = ` function that the call passes by name.
fn listeners(script: &str) -> Vec<(String, String)> {
    const CALL: &str = "addEventListener(";
    let mut found = Vec::new();
    for (at, _) in script.match_indices(CALL) {
        let call = &script[at + CALL.len()..];
        let event = call.split('"').nth(1).unwrap_or_default().to_owned();
        let handler = call.split_once(',').map_or("", |(_, rest)| rest.trim_start());
        let name: String = handler
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        let text = if name.is_empty() {
            handler
        } else {
            let definition = format!("const {name} = ");
            script.split_once(&definition).map_or("", |(_, rest)| rest)
        };
        found.push((event, text.to_owned()));
    }
    found
}

/// True when the first statement of the arrow function `handler` returns for an event that
/// the user did not make: `(e) => { if (!e.isTrusted) return; ...`.
fn trusted_only(handler: &str) -> bool {
    let Some((params, body)) = handler.split_once("=>") else {
        return false;
    };
    let param = params.trim().trim_start_matches('(').trim_end_matches(')');
    let first = body.trim_start().trim_start_matches('{').trim_start();
    !param.is_empty() && first.starts_with(&format!("if (!{param}.isTrusted) return;"))
}

#[test]
fn t1_every_listener_of_the_input_script_ignores_events_that_a_page_made() {
    for (content, count) in [(true, 5), (false, 1)] {
        let found = listeners(&input_script(content));
        assert_eq!(found.len(), count, "content={content}: {found:?}");
        for (event, handler) in &found {
            assert!(
                trusted_only(handler),
                "content={content}: the {event} listener acts on a page-made event"
            );
        }
    }
}

#[test]
fn t1_every_listener_of_the_hover_script_ignores_events_that_a_page_made() {
    let found = listeners(HOVER_SCRIPT);
    assert_eq!(found.len(), 2, "{found:?}");
    for (event, handler) in &found {
        assert!(
            trusted_only(handler),
            "the {event} listener acts on a page-made event"
        );
    }
}

#[test]
fn t1_the_listener_check_finds_a_listener_without_the_trusted_test() {
    let script = r#"window.addEventListener("click", (e) => { post(e); }, true);
  const on = (ev) => { if (!ev.isTrusted) return; post(ev); };
  window.addEventListener("keydown", on, true);"#;
    let found = listeners(script);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].0, "click");
    assert!(!trusted_only(&found[0].1));
    assert_eq!(found[1].0, "keydown");
    assert!(trusted_only(&found[1].1));
}

// ---------------------------------------------------------------------------------------------
// Key codes (K1, K2): the key that types the character is the code
// ---------------------------------------------------------------------------------------------

#[test]
fn k1_every_letter_of_either_case_gives_its_key_code() {
    for c in "abcdefghijklmnopqrstuvwxyz".chars() {
        let want = format!("Key{}", c.to_ascii_uppercase());
        assert_eq!(code_of_char(c), Some(want.clone()), "{c}");
        assert_eq!(code_of_char(c.to_ascii_uppercase()), Some(want), "{c}");
    }
}

#[test]
fn k1_the_key_that_types_z_is_key_z() {
    assert_eq!(code("z"), Some("KeyZ".to_owned()));
    assert_eq!(code("Z"), Some("KeyZ".to_owned()));
}

#[test]
fn k1_every_digit_gives_its_digit_code() {
    for c in "0123456789".chars() {
        assert_eq!(code_of_char(c), Some(format!("Digit{c}")), "{c}");
    }
}

#[test]
fn k1_the_punctuation_of_the_shortcuts_gives_its_key_code() {
    let table = [
        ("[", "BracketLeft"),
        ("]", "BracketRight"),
        ("=", "Equal"),
        ("+", "Equal"),
        ("-", "Minus"),
        (",", "Comma"),
        (".", "Period"),
    ];
    for (text, want) in table {
        assert_eq!(code(text), Some(want.to_owned()), "{text}");
    }
}

#[test]
fn k5_a_character_that_is_no_shortcut_key_gives_no_code() {
    for text in ["/", ";", " ", "\\", "`", "~", "\t", "\n", "_", "*", "!"] {
        assert_eq!(code(text), None, "{text:?}");
    }
}

#[test]
fn k1_the_virtual_keys_of_the_digits_give_the_digit_codes() {
    for n in 0..10u32 {
        assert_eq!(
            code_of_virtual_key(0x30 + n),
            Some(format!("Digit{n}")),
            "{n}"
        );
    }
}

#[test]
fn k1_the_virtual_keys_of_the_letters_give_the_key_codes() {
    for (n, c) in "ABCDEFGHIJKLMNOPQRSTUVWXYZ".chars().enumerate() {
        let vk = 0x41 + u32::try_from(n).unwrap_or(u32::MAX);
        assert_eq!(code_of_virtual_key(vk), Some(format!("Key{c}")), "{vk:#x}");
    }
}

#[test]
fn k1_the_virtual_keys_of_the_function_keys_give_f1_to_f12() {
    for n in 0..12u32 {
        assert_eq!(
            code_of_virtual_key(0x70 + n),
            Some(format!("F{}", n + 1)),
            "{n}"
        );
    }
}

#[test]
fn k5_the_virtual_keys_of_the_editing_and_navigation_keys_give_their_codes() {
    let table = [
        (0x08, "Backspace"),
        (0x09, "Tab"),
        (0x1B, "Escape"),
        (0x21, "PageUp"),
        (0x22, "PageDown"),
        (0x23, "End"),
        (0x24, "Home"),
        (0x25, "ArrowLeft"),
        (0x26, "ArrowUp"),
        (0x27, "ArrowRight"),
        (0x28, "ArrowDown"),
        (0x2E, "Delete"),
    ];
    for (vk, want) in table {
        assert_eq!(code_of_virtual_key(vk), Some(want.to_owned()), "{vk:#x}");
    }
}

#[test]
fn k1_a_virtual_key_outside_the_table_gives_no_code() {
    let outside = [
        0x00,
        0x07,
        0x0A,
        0x0D,
        0x10,
        0x11,
        0x12,
        0x1A,
        0x1C,
        0x20,
        0x29,
        0x2D,
        0x2F,
        0x3A,
        0x40,
        0x5B,
        0x60,
        0x6F,
        0x7C,
        0x87,
        0xBA,
        0x100,
        u32::MAX,
    ];
    for vk in outside {
        assert_eq!(code_of_virtual_key(vk), None, "{vk:#x}");
    }
}
