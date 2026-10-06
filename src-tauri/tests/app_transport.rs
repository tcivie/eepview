// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! B1 of `docs/wiki/browser-shell.md`, "macOS bundle: App Transport Security".
//!
//! Eepsites are plain `http://` pages, and the bundled app on macOS refuses every
//! `http://` page load unless App Transport Security allows web content. Tauri copies the
//! keys of `src-tauri/Info.plist` into the bundle `Info.plist`, so that file must:
//!
//! 1. exist as an XML property list whose top dict holds `NSAppTransportSecurity`, a dict
//!    that holds `NSAllowsArbitraryLoadsInWebContent` set to `<true/>`;
//! 2. hold no other key in that `NSAppTransportSecurity` dict, never
//!    `NSAllowsArbitraryLoads`, `NSAllowsArbitraryLoadsForMedia`, `NSAllowsLocalNetworking`
//!    or `NSExceptionDomains` (the exemption is for web view content only, nothing wider);
//! 3. hold only the `NSAppTransportSecurity` key in its top dict, so no other bundle key
//!    is overridden by this file.
//!
//! The plist is parsed by hand with `std` only: comments are stripped, the tags are
//! tokenised, and each `<key>` is recorded with its path of parent keys and the tag of its
//! value.

use std::fs;
use std::path::PathBuf;

/// The one top level key this file may hold.
const ATS: &str = "NSAppTransportSecurity";

/// The one key the ATS dict may hold.
const WEB_CONTENT: &str = "NSAllowsArbitraryLoadsInWebContent";

/// One piece of the XML: a tag or the trimmed text between two tags.
enum Token {
    Open(String),
    Close(String),
    Empty(String),
    Text(String),
}

/// One `<key>` of the plist.
struct Entry {
    /// The parent keys then the key, joined by `/`. A top level key has no `/`.
    path: String,
    /// The tag name of the value that follows the key, or empty when none does.
    value: String,
}

/// Reads `src-tauri/Info.plist` or says why it cannot.
fn read_plist() -> Result<String, String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Info.plist");
    fs::read_to_string(&path).map_err(|err| {
        format!(
            "B1: cannot read {}: {err}. The macOS bundle needs this file to allow http:// web content.",
            path.display()
        )
    })
}

/// Drops every `<!-- ... -->` block, such as the SPDX header.
fn strip_comments(xml: &str) -> String {
    let mut out = String::new();
    let mut rest = xml;
    while let Some((before, after)) = rest.split_once("<!--") {
        out.push_str(before);
        rest = after.split_once("-->").map_or("", |(_, tail)| tail);
    }
    out.push_str(rest);
    out
}

/// The tag name: the first word, without attributes.
fn first_word(tag: &str) -> String {
    tag.split_whitespace().next().unwrap_or("").to_owned()
}

/// Turns the inside of `<...>` into a token. The XML declaration and the doctype give none.
fn classify(tag: &str) -> Option<Token> {
    if tag.starts_with(['?', '!']) {
        return None;
    }
    if let Some(name) = tag.strip_prefix('/') {
        return Some(Token::Close(first_word(name)));
    }
    match tag.strip_suffix('/') {
        Some(inner) => Some(Token::Empty(first_word(inner))),
        None => Some(Token::Open(first_word(tag))),
    }
}

/// Splits comment free XML into tags and non blank text.
fn tokenize(xml: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    for piece in xml.split('<').skip(1) {
        let (tag, text) = piece.split_once('>').unwrap_or((piece, ""));
        tokens.extend(classify(tag));
        let text = text.trim();
        tokens.extend((!text.is_empty()).then(|| Token::Text(text.to_owned())));
    }
    tokens
}

/// The tag name of a value token, or empty when there is no tag.
fn value_tag(token: Option<&Token>) -> String {
    match token {
        Some(Token::Open(name) | Token::Empty(name)) => name.clone(),
        _ => String::new(),
    }
}

/// Records the key that opens at `index`. The key text is the next token, the value the
/// token after the closing `</key>`.
fn key_entry(tokens: &[Token], index: usize, owners: &[String]) -> Option<Entry> {
    let Some(Token::Text(name)) = tokens.get(index + 1) else {
        return None;
    };
    let mut parts: Vec<&str> = owners.iter().skip(1).map(String::as_str).collect();
    parts.push(name);
    Some(Entry {
        path: parts.join("/"),
        value: value_tag(tokens.get(index + 3)),
    })
}

/// Every `<key>` of the plist, with the path of the dicts it sits in.
fn entries(xml: &str) -> Vec<Entry> {
    let tokens = tokenize(&strip_comments(xml));
    // One owner key per open dict. The top dict is owned by the empty key.
    let mut owners: Vec<String> = Vec::new();
    let mut found: Vec<Entry> = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        match token {
            Token::Open(name) if name == "dict" => owners.push(last_key(&found)),
            Token::Close(name) if name == "dict" => drop(owners.pop()),
            Token::Open(name) if name == "key" => found.extend(key_entry(&tokens, index, &owners)),
            _ => {}
        }
    }
    found
}

/// The name of the last key seen, which owns a dict that opens next.
fn last_key(found: &[Entry]) -> String {
    found
        .last()
        .and_then(|entry| entry.path.rsplit('/').next())
        .unwrap_or("")
        .to_owned()
}

/// Reads and parses the plist, and checks that it is an XML property list.
fn load_entries() -> Result<Vec<Entry>, String> {
    let xml = read_plist()?;
    let body = strip_comments(&xml);
    if !body.contains("<plist") || !body.contains("<dict>") {
        return Err(
            "B1: Info.plist is not an XML property list (no <plist> with a <dict>)".to_owned(),
        );
    }
    Ok(entries(&xml))
}

/// The tag name of the value of the key at `path`.
fn value_at<'a>(all: &'a [Entry], path: &str) -> Option<&'a str> {
    all.iter()
        .find(|entry| entry.path == path)
        .map(|entry| entry.value.as_str())
}

/// The names of the keys that are direct children of the dict at `parent`. An empty
/// `parent` is the top dict.
fn children<'a>(all: &'a [Entry], parent: &str) -> Vec<&'a str> {
    all.iter()
        .filter_map(|entry| {
            if parent.is_empty() {
                return (!entry.path.contains('/')).then_some(entry.path.as_str());
            }
            let name = entry.path.strip_prefix(parent)?.strip_prefix('/')?;
            (!name.contains('/')).then_some(name)
        })
        .collect()
}

#[test]
fn b1_the_bundle_allows_http_web_content() -> Result<(), String> {
    // Given the keys of src-tauri/Info.plist
    let all = load_entries()?;
    // When the ATS dict and its web content key are looked up
    let ats = value_at(&all, ATS);
    let web = value_at(&all, &format!("{ATS}/{WEB_CONTENT}"));
    // Then web content may load over plain http
    assert_eq!(
        ats,
        Some("dict"),
        "B1: {ATS} must be a dict in the top dict of Info.plist"
    );
    assert_eq!(
        web,
        Some("true"),
        "B1: {ATS} must hold {WEB_CONTENT} = <true/>, or the macOS bundle refuses http:// eepsites"
    );
    Ok(())
}

#[test]
fn b1_the_exemption_covers_web_content_only() -> Result<(), String> {
    // Given the keys of src-tauri/Info.plist
    let all = load_entries()?;
    // When the keys of the ATS dict are collected
    let keys = children(&all, ATS);
    // Then only the web content key is there, never a wider exemption
    assert_eq!(
        keys,
        vec![WEB_CONTENT],
        "B1: the {ATS} dict must hold only {WEB_CONTENT}; \
         never NSAllowsArbitraryLoads, NSAllowsArbitraryLoadsForMedia, \
         NSAllowsLocalNetworking or NSExceptionDomains"
    );
    Ok(())
}

#[test]
fn b1_the_plist_overrides_only_app_transport_security() -> Result<(), String> {
    // Given the keys of src-tauri/Info.plist
    let all = load_entries()?;
    // When the keys of the top dict are collected
    let keys = children(&all, "");
    // Then it holds the ATS key alone, so no other bundle key is overridden
    assert_eq!(
        keys,
        vec![ATS],
        "B1: the top dict of Info.plist must hold only {ATS}, so no other bundle key is overridden"
    );
    Ok(())
}
