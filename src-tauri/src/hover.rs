// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The status bubble text: link targets under the mouse and "Loading <host>…". Pure.

use tauri::Url;

use crate::nav::is_allowed_web;

/// Longest text shown; longer text loses its middle.
pub const MAX_CHARS: usize = 80;
/// Delay before the bubble hides, in ms (it shows at once).
pub const HIDE_DELAY_MS: u64 = 150;

/// What the bubble shows.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct HoverText {
    /// Text only, never HTML.
    pub text: String,
    /// The link cannot be followed (not an I2P site).
    pub blocked: bool,
}

/// The bubble text for a link target. `None` for an empty target.
#[must_use]
pub fn link(target: &str) -> Option<HoverText> {
    let target = target.trim();
    if target.is_empty() {
        return None;
    }
    let Ok(url) = Url::parse(target) else {
        return Some(blocked(&clean(target)));
    };
    if !is_allowed_web(&url) {
        let host = url
            .host_str()
            .map_or_else(|| format!("{}:", url.scheme()), str::to_owned);
        return Some(blocked(&host));
    }
    Some(HoverText {
        text: shorten(&clean(&decode(url.as_str()))),
        blocked: false,
    })
}

/// The bubble text while a page loads.
#[must_use]
pub fn loading(url: &str) -> Option<HoverText> {
    let host = Url::parse(url).ok()?.host_str()?.to_owned();
    Some(HoverText {
        text: shorten(&format!("Loading {host}…")),
        blocked: false,
    })
}

fn blocked(what: &str) -> HoverText {
    HoverText {
        text: shorten(&format!("Blocked: {what}")),
        blocked: true,
    }
}

/// Decodes `%XX` escapes for display; invalid UTF-8 shows as U+FFFD.
#[must_use]
pub fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes.get(i + 1..i + 3).and_then(hex_byte);
        match (bytes[i], hex) {
            (b'%', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_byte(pair: &[u8]) -> Option<u8> {
    let text = std::str::from_utf8(pair).ok()?;
    u8::from_str_radix(text, 16).ok()
}

/// Drops control and bidi-override characters, which could disguise the target.
fn clean(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() && !matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}'))
        .collect()
}

/// Cuts the middle of long text: `start…end`, at most [`MAX_CHARS`] characters.
#[must_use]
pub fn shorten(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= MAX_CHARS {
        return text.to_owned();
    }
    let head = MAX_CHARS * 2 / 3;
    let tail = MAX_CHARS - head - 1;
    let mut out: String = chars[..head].iter().collect();
    out.push('…');
    out.extend(&chars[chars.len() - tail..]);
    out
}

/// Shows at once, hides after [`HIDE_DELAY_MS`] unless something new shows first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Debounce {
    generation: u64,
    shown: Option<HoverText>,
}

impl Debounce {
    /// New text: show it now. Returns the text when it changed.
    pub fn show(&mut self, text: HoverText) -> Option<HoverText> {
        self.generation += 1;
        if self.shown.as_ref() == Some(&text) {
            return None;
        }
        self.shown = Some(text.clone());
        Some(text)
    }

    /// The mouse left: returns the generation to hide at after the delay.
    pub fn leave(&mut self) -> u64 {
        self.generation += 1;
        self.generation
    }

    /// The delay passed. True when the bubble must hide now.
    pub fn expire(&mut self, generation: u64) -> bool {
        let hide = generation == self.generation && self.shown.is_some();
        if hide {
            self.shown = None;
        }
        hide
    }

    /// The text on screen.
    #[must_use]
    pub fn shown(&self) -> Option<&HoverText> {
        self.shown.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i2p_links_show_decoded() {
        let t = link("http://stats.i2p/a%20b?q=%C3%BC").unwrap();
        assert_eq!(t.text, "http://stats.i2p/a b?q=ü");
        assert!(!t.blocked);
        assert_eq!(link("  "), None);
    }

    #[test]
    fn other_targets_are_blocked() {
        let t = link("http://example.com/x").unwrap();
        assert_eq!(
            t,
            HoverText {
                text: "Blocked: example.com".into(),
                blocked: true
            }
        );
        assert_eq!(
            link("javascript:alert(1)").unwrap().text,
            "Blocked: javascript:"
        );
        assert_eq!(
            link("http://127.0.0.1:7657/").unwrap().text,
            "Blocked: 127.0.0.1"
        );
        assert_eq!(link("not a url").unwrap().text, "Blocked: not a url");
    }

    #[test]
    fn spoofing_characters_are_dropped() {
        let t = link("http://stats.i2p/%E2%80%AEtxt.exe%0A").unwrap();
        assert_eq!(t.text, "http://stats.i2p/txt.exe");
    }

    #[test]
    fn long_text_loses_its_middle() {
        let long = format!("http://stats.i2p/{}", "a".repeat(200));
        let short = shorten(&long);
        assert_eq!(short.chars().count(), MAX_CHARS);
        assert!(short.starts_with("http://stats.i2p/") && short.contains('…'));
        assert_eq!(shorten("short"), "short");
    }

    #[test]
    fn decode_edge_cases() {
        assert_eq!(decode("%"), "%");
        assert_eq!(decode("%zz%4"), "%zz%4");
        assert_eq!(decode("%ff"), "\u{FFFD}");
    }

    #[test]
    fn loading_text() {
        assert_eq!(
            loading("http://stats.i2p/x").unwrap().text,
            "Loading stats.i2p…"
        );
        assert_eq!(
            loading("eepview://home").map(|t| t.text),
            Some("Loading home…".into())
        );
        assert_eq!(loading("::"), None);
    }

    #[test]
    fn debounce() {
        let mut d = Debounce::default();
        let a = link("http://a.i2p/").unwrap();
        assert_eq!(d.show(a.clone()), Some(a.clone()));
        assert_eq!(d.show(a.clone()), None);
        let g = d.leave();
        assert_eq!(d.show(a.clone()), None);
        assert!(!d.expire(g));
        let g2 = d.leave();
        assert!(d.expire(g2));
        assert!(d.shown().is_none());
        assert!(!d.expire(g2));
    }
}
