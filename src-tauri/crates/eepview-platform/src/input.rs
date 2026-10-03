// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The input that the engines report (docs/wiki/links-and-shortcuts.md): link clicks, keys,
//! mouse buttons and context menu requests, as plain Rust data. The app crate decides what
//! each one does; this module only names them, and parses the messages of the macOS input
//! script and the Windows virtual keys.

/// The modifier keys held during a click or a key press. One bit per key, so a combination
/// is one small value.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Keys {
    bits: u8,
}

const META: u8 = 1;
const CTRL: u8 = 2;
const ALT: u8 = 4;
const SHIFT: u8 = 8;

impl Keys {
    /// Command on macOS, the Windows (Super) key elsewhere.
    #[must_use]
    pub const fn meta(self) -> bool {
        self.bits & META != 0
    }

    /// Control.
    #[must_use]
    pub const fn ctrl(self) -> bool {
        self.bits & CTRL != 0
    }

    /// Alt (Option on macOS).
    #[must_use]
    pub const fn alt(self) -> bool {
        self.bits & ALT != 0
    }

    /// Shift.
    #[must_use]
    pub const fn shift(self) -> bool {
        self.bits & SHIFT != 0
    }

    /// These keys, with Command (macOS) or Super held or not.
    #[must_use]
    pub const fn with_meta(self, down: bool) -> Self {
        self.with(META, down)
    }

    /// These keys, with Control held or not.
    #[must_use]
    pub const fn with_ctrl(self, down: bool) -> Self {
        self.with(CTRL, down)
    }

    /// These keys, with Alt (Option) held or not.
    #[must_use]
    pub const fn with_alt(self, down: bool) -> Self {
        self.with(ALT, down)
    }

    /// These keys, with Shift held or not.
    #[must_use]
    pub const fn with_shift(self, down: bool) -> Self {
        self.with(SHIFT, down)
    }

    const fn with(self, bit: u8, down: bool) -> Self {
        let bits = if down {
            self.bits | bit
        } else {
            self.bits & !bit
        };
        Self { bits }
    }
}

/// The mouse button of a click.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Button {
    /// A keyboard activation, such as Enter on a link.
    None,
    /// The primary (left) button.
    Primary,
    /// The middle button or wheel.
    Middle,
    /// The secondary (right) button.
    Secondary,
    /// The back button.
    Back,
    /// The forward button.
    Forward,
}

/// What is under the pointer when a context menu is asked for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hit {
    /// The absolute URL of the link under the pointer.
    pub link: Option<String>,
    /// The absolute URL of the image under the pointer.
    pub image: Option<String>,
    /// Text is selected.
    pub selection: bool,
    /// The pointer is in a text field.
    pub editable: bool,
}

/// One input event of a webview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// A link was activated with a button or modifiers that may open a tab. The engine has not
    /// followed it.
    Link {
        /// The absolute URL of the link.
        url: String,
        /// The held modifiers.
        keys: Keys,
        /// The button, or [`Button::None`] for a keyboard activation.
        button: Button,
    },
    /// A key press, as a `KeyboardEvent.code` name of the key's character on the active
    /// layout (`KeyT`, `Digit1`, `Escape`, `F5`).
    Key {
        /// The key name.
        code: String,
        /// The held modifiers.
        keys: Keys,
    },
    /// The mouse back or forward button.
    Mouse(Button),
    /// A context menu request.
    Menu(Hit),
}

/// An editing command that the engine carries out itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Native {
    /// Undo.
    Undo,
    /// Redo.
    Redo,
    /// Cut.
    Cut,
    /// Copy the selection.
    Copy,
    /// Paste.
    Paste,
    /// Select all.
    SelectAll,
    /// Copy the image under the pointer.
    CopyImage,
}

/// One entry of a context menu that the app built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuEntry {
    /// A command.
    Item {
        /// The id that the `chosen` hook gets.
        id: String,
        /// The label.
        label: String,
        /// False for a grey item.
        enabled: bool,
        /// The engine command behind the item, when the engine does the work.
        native: Option<Native>,
    },
    /// A separator line.
    Separator,
}

/// The answer of the input hook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    /// The engine handles the event as usual.
    Pass,
    /// The app handled the event; the engine drops it.
    Consume,
    /// Show this menu (Windows and Linux rebuild the engine menu from it). An empty list shows
    /// no menu.
    Menu(Vec<MenuEntry>),
}

/// The callbacks of [`crate::on_input`].
pub struct Hooks {
    /// True for a page webview (`tab-*`, the internal pages, the router console): it reports
    /// links, Esc and the mouse buttons. Every webview reports context menu requests and, on
    /// Windows, key presses.
    pub content: bool,
    /// Gets every input event.
    pub input: Box<dyn Fn(Input) -> Reply>,
    /// Gets the id of the menu item that was chosen.
    pub chosen: Box<dyn Fn(&str)>,
}

/// The name of the macOS message handler of the input script.
pub const INPUT_CHANNEL: &str = "eepviewInput";

/// The context menu part of the input script, for every webview. It cancels the engine
/// menu and posts `menu<TAB>flags<TAB>link<TAB>image`, with `s` (selection) and `e` (text
/// field) in the flags.
const MENU_SCRIPT: &str = r#"
  const post = (m) => {
    try { window.webkit.messageHandlers.eepviewInput.postMessage(m); } catch (e) {}
  };
  const keys = (e) =>
    (e.metaKey ? "m" : "") + (e.ctrlKey ? "c" : "") + (e.altKey ? "a" : "") + (e.shiftKey ? "s" : "");
  const elementOf = (n) => (n && n.nodeType === 1 ? n : n && n.parentElement);
  const TEXT = ["", "text", "search", "url", "tel", "email", "password", "number"];
  const field = (el) => {
    const f = el.closest("input, textarea");
    if (!f || f.disabled || f.readOnly) return null;
    if (f.tagName === "TEXTAREA") return f;
    return TEXT.includes((f.getAttribute("type") || "").toLowerCase()) ? f : null;
  };
  const fieldSelected = (f) => {
    try { return f.selectionStart !== f.selectionEnd; } catch (e) { return false; }
  };
  let lastImage = null;
  window.__eepviewSelectImage = () => {
    if (!lastImage || !lastImage.isConnected) return false;
    const range = document.createRange();
    range.selectNode(lastImage);
    const s = window.getSelection();
    s.removeAllRanges();
    s.addRange(range);
    return true;
  };
  window.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    const el = elementOf(e.target);
    if (!el) return;
    const f = field(el);
    const editable = Boolean(f) || el.isContentEditable;
    const selected = f ? fieldSelected(f) : String(window.getSelection() || "").length > 0;
    const a = el.closest("a[href], area[href]");
    const img = el.closest("img");
    lastImage = img;
    const flags = (selected ? "s" : "") + (editable ? "e" : "");
    const src = img ? String(img.currentSrc || img.src) : "";
    post(["menu", flags, a ? String(a.href) : "", src].join("\t"));
  }, true);
"#;

/// The page part of the input script: modified clicks on links (cancelled, then posted as
/// `link<TAB>button<TAB>keys<TAB>url`), Esc (`key<TAB>Escape<TAB>keys`) and the mouse back
/// and forward buttons (`mouse<TAB>3` or `4`).
const CONTENT_SCRIPT: &str = r#"
  const opensTab = (e) =>
    e.button === 1 || (e.button === 0 && !e.ctrlKey && (e.metaKey || e.shiftKey));
  const onLink = (e) => {
    const el = elementOf(e.target);
    const a = el && el.closest("a[href], area[href]");
    if (!a || !opensTab(e)) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    const button = e.button === 0 && e.detail === 0 ? "none" : String(e.button);
    post(["link", button, keys(e), String(a.href)].join("\t"));
  };
  window.addEventListener("click", onLink, true);
  window.addEventListener("auxclick", onLink, true);
  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape") post(["key", "Escape", keys(e)].join("\t"));
  }, true);
  window.addEventListener("mouseup", (e) => {
    if (e.button !== 3 && e.button !== 4) return;
    e.preventDefault();
    post(["mouse", String(e.button)].join("\t"));
  }, true);
"#;

/// The input script of a webview (macOS). It runs in the private script world, so page
/// scripts can neither see nor call it, and it runs with page JavaScript off. It runs once
/// per document, even when it is also evaluated by hand.
#[must_use]
pub fn input_script(content: bool) -> String {
    let page = if content { CONTENT_SCRIPT } else { "" };
    format!(
        "(() => {{\n  if (window.__eepviewInput) return;\n  window.__eepviewInput = true;\n{MENU_SCRIPT}{page}}})();"
    )
}

/// The modifiers of a message: `m`, `c`, `a` and `s` for meta, ctrl, alt and shift.
#[must_use]
pub fn keys_of(text: &str) -> Keys {
    let held = |flag: &str| text.contains(flag);
    Keys::default()
        .with_meta(held("m"))
        .with_ctrl(held("c"))
        .with_alt(held("a"))
        .with_shift(held("s"))
}

/// The button of a DOM `MouseEvent.button` number, or `none` for a keyboard activation.
#[must_use]
pub fn dom_button(text: &str) -> Option<Button> {
    Some(match text {
        "none" => Button::None,
        "0" => Button::Primary,
        "1" => Button::Middle,
        "2" => Button::Secondary,
        "3" => Button::Back,
        "4" => Button::Forward,
        _ => return None,
    })
}

/// A message of the input script as an [`Input`]; `None` for a message it does not know.
#[must_use]
pub fn parse_message(text: &str) -> Option<Input> {
    let parts: Vec<&str> = text.split('\t').collect();
    match parts.as_slice() {
        ["link", button, keys, url] if !url.is_empty() => Some(Input::Link {
            url: (*url).to_owned(),
            keys: keys_of(keys),
            button: dom_button(button)?,
        }),
        ["key", code, keys] if !code.is_empty() => Some(Input::Key {
            code: (*code).to_owned(),
            keys: keys_of(keys),
        }),
        ["mouse", button] => match dom_button(button)? {
            b @ (Button::Back | Button::Forward) => Some(Input::Mouse(b)),
            _ => None,
        },
        ["menu", flags, link, image] => Some(Input::Menu(Hit {
            link: non_empty(link),
            image: non_empty(image),
            selection: flags.contains(SELECTED),
            editable: flags.contains(EDITABLE),
        })),
        _ => None,
    }
}

/// The menu flag for selected text.
const SELECTED: &str = "s";
/// The menu flag for a text field.
const EDITABLE: &str = "e";

fn non_empty(text: &str) -> Option<String> {
    (!text.is_empty()).then(|| text.to_owned())
}

/// The `KeyboardEvent.code` name of a character key: letters, digits and the punctuation of
/// the shortcut table. `+` names the zoom key of layouts that have no `=` key.
#[must_use]
pub fn code_of_char(c: char) -> Option<String> {
    let upper = c.to_ascii_uppercase();
    if upper.is_ascii_uppercase() {
        return Some(format!("Key{upper}"));
    }
    if c.is_ascii_digit() {
        return Some(format!("Digit{c}"));
    }
    let name = match c {
        '[' => "BracketLeft",
        ']' => "BracketRight",
        '=' | '+' => "Equal",
        '-' => "Minus",
        ',' => "Comma",
        '.' => "Period",
        _ => return None,
    };
    Some(name.to_owned())
}

/// The `KeyboardEvent.code` name of a Windows virtual key that has no character, or of a
/// letter or digit key (their virtual keys follow the layout).
#[must_use]
pub fn code_of_virtual_key(vk: u32) -> Option<String> {
    let name = match vk {
        0x08 => "Backspace",
        0x09 => "Tab",
        0x1B => "Escape",
        0x21 => "PageUp",
        0x22 => "PageDown",
        0x23 => "End",
        0x24 => "Home",
        0x25 => "ArrowLeft",
        0x26 => "ArrowUp",
        0x27 => "ArrowRight",
        0x28 => "ArrowDown",
        0x2E => "Delete",
        0x30..=0x39 | 0x41..=0x5A => return char::from_u32(vk).and_then(code_of_char),
        0x70..=0x7B => return Some(format!("F{}", vk - 0x6F)),
        _ => return None,
    };
    Some(name.to_owned())
}
