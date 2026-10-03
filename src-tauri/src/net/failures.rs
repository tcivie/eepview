// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The I2P pages that failed, as the gatekeeper saw them. A page that fails is shown but not
//! saved in history. The gatekeeper knows the request URL and the answer; the engine reports
//! only a URL when the page finishes, so the two meet here by exact URL.

use std::collections::VecDeque;
use std::sync::{Mutex, PoisonError};

use tauri::Url;

/// Most failed URLs kept. The oldest goes first.
const MAX_KEPT: usize = 64;

/// The URLs whose last answer was a failure.
#[derive(Debug, Default)]
pub struct Failures {
    urls: Mutex<VecDeque<String>>,
}

/// The key of a URL: normalised, with no fragment.
fn key(url: &str) -> Option<String> {
    let mut url = Url::parse(url).ok()?;
    url.set_fragment(None);
    Some(url.into())
}

/// True for a `5xx` status line (`HTTP/1.1 503 Service Unavailable`).
#[must_use]
pub fn is_server_error(status_line: &str) -> bool {
    status_line
        .split(' ')
        .nth(1)
        .is_some_and(|code| code.len() == 3 && code.starts_with('5'))
}

impl Failures {
    /// Records the latest answer for `url`: a failure is kept, any other answer clears it.
    pub fn note(&self, url: &str, failed: bool) {
        let Some(key) = key(url) else {
            return;
        };
        let mut urls = self.urls.lock().unwrap_or_else(PoisonError::into_inner);
        urls.retain(|u| *u != key);
        if failed {
            urls.push_back(key);
        }
        while urls.len() > MAX_KEPT {
            urls.pop_front();
        }
    }

    /// True once when the last answer for `url` was a failure. The flag clears when read.
    pub fn take(&self, url: &str) -> bool {
        let Some(key) = key(url) else {
            return false;
        };
        let mut urls = self.urls.lock().unwrap_or_else(PoisonError::into_inner);
        let before = urls.len();
        urls.retain(|u| *u != key);
        urls.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failure_is_read_once() {
        let f = Failures::default();
        f.note("http://a.i2p/x?q=1", true);
        assert!(f.take("http://a.i2p/x?q=1#top"));
        assert!(!f.take("http://a.i2p/x?q=1"));
    }

    #[test]
    fn a_later_good_answer_clears_it() {
        let f = Failures::default();
        f.note("http://a.i2p/", true);
        f.note("http://a.i2p/", false);
        assert!(!f.take("http://a.i2p/"));
    }

    #[test]
    fn the_oldest_failure_goes_first() {
        let f = Failures::default();
        for n in 0..=MAX_KEPT {
            f.note(&format!("http://a.i2p/{n}"), true);
        }
        assert!(!f.take("http://a.i2p/0"));
        assert!(f.take(&format!("http://a.i2p/{MAX_KEPT}")));
    }

    #[test]
    fn bad_urls_are_ignored() {
        let f = Failures::default();
        f.note("not a url", true);
        assert!(!f.take("not a url"));
    }

    #[test]
    fn server_errors_are_the_5xx_codes() {
        assert!(is_server_error("HTTP/1.1 503 Service Unavailable"));
        assert!(is_server_error("HTTP/1.0 500 x"));
        assert!(!is_server_error("HTTP/1.1 404 Not Found"));
        assert!(!is_server_error("HTTP/1.1 200 OK"));
        assert!(!is_server_error("garbage"));
    }
}
