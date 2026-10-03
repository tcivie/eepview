// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The one host predicate (ADR 0001). The gatekeeper (L1), the engine rules (L3) and the
//! navigation guard (L4) all call [`is_i2p_host`].

/// Shortest `*.b32.i2p` label: 52 base32 characters (a 256-bit hash).
const B32_MIN_LEN: usize = 52;
/// The base32 alphabet of I2P destination hashes.
const BASE32: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";

/// True for a normalised I2P host name: lower-case ASCII labels of `a-z`, `0-9` and `-`,
/// ending in `.i2p`, with no empty label, no trailing dot, no port, no user info and no
/// IDN (`xn--`) label. A `*.b32.i2p` label must be base32 of at least 52 characters.
#[must_use]
pub fn is_i2p_host(host: &str) -> bool {
    let Some(name) = host.strip_suffix(".i2p") else {
        return false;
    };
    if name.is_empty() || !name.split('.').all(is_plain_label) {
        return false;
    }
    match name.strip_suffix(".b32") {
        Some(label) => is_b32_label(label),
        None => name != "b32",
    }
}

/// A DNS label (RFC 1123): letters, digits and `-`, never `-` at either end, and no IDN `xn--`.
fn is_plain_label(label: &str) -> bool {
    !label.is_empty()
        && !label.starts_with('-')
        && !label.ends_with('-')
        && !label.starts_with("xn--")
        && label
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn is_b32_label(label: &str) -> bool {
    label.len() >= B32_MIN_LEN && label.bytes().all(|b| BASE32.contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    const B32: &str = "ukeu3k5oycgaauneqgtnvselmt4yemvoilkln7jpvamvfx7dnkdq.b32.i2p";

    #[test]
    fn table() {
        let cases: &[(&str, bool)] = &[
            ("stats.i2p", true),
            ("i2p-projekt.i2p", true),
            ("forum.stats.i2p", true),
            ("a1.i2p", true),
            (B32, true),
            ("i2p", false),
            (".i2p", false),
            ("foo..i2p", false),
            ("foo.i2p.", false),
            ("Foo.i2p", false),
            ("FOO.I2P", false),
            ("foo.i2p.evil.com", false),
            ("foo.i2p:80", false),
            ("a@foo.i2p", false),
            ("xn--bcher-kva.i2p", false),
            ("a.-xn--.i2p", false),
            ("-foo.i2p", false),
            ("foo-.i2p", false),
            ("a.-b.i2p", false),
            ("a-.b.i2p", false),
            ("bücher.i2p", false),
            ("foo.\u{456}2p", false),
            ("foo_bar.i2p", false),
            ("foo bar.i2p", false),
            ("127.0.0.1", false),
            ("localhost", false),
            ("[::1]", false),
            ("b32.i2p", false),
            ("short.b32.i2p", false),
            ("", false),
        ];
        for (host, want) in cases {
            assert_eq!(is_i2p_host(host), *want, "{host:?}");
        }
        assert!(!is_i2p_host(&B32.replace('u', "1")));
        assert!(!is_i2p_host(&B32.replace('u', "U")));
    }
}
