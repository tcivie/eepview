// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Gatekeeper trace on stderr, only when `EEPVIEW_LOG` is set: one line for each request head,
//! answer and refusal. The leak test keeps it in `app.<run>.log`, so a request that never
//! reached the router is explained by the log.

/// The line of an event.
fn text(what: &str) -> String {
    format!("[eepview] gatekeeper {what}")
}

/// Prints `what` when `EEPVIEW_LOG` is set.
pub fn line(what: &str) {
    if std::env::var_os("EEPVIEW_LOG").is_some() {
        eprintln!("{}", text(what));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_name_the_gatekeeper() {
        assert_eq!(text("GET x"), "[eepview] gatekeeper GET x");
        line("test line");
    }
}
