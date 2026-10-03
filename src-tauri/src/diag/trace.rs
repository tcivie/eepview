// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! A test trace on stderr, only when `EEPVIEW_LOG` is set: one line for each gatekeeper
//! request head, answer, refusal and failed connection, and one for each first load of a tab
//! webview. The leak test sets the variable and keeps the lines in `app.<run>.log`, so a page
//! that never reached the router is explained by the log. The trace is never written to the
//! diagnostics log files and never goes into a bug report.

/// The line of an event from `part` (`gatekeeper` or `load`).
fn text(part: &str, what: &str) -> String {
    format!("[eepview] {part} {what}")
}

fn print(line: &str) {
    if std::env::var_os("EEPVIEW_LOG").is_some() {
        eprintln!("{line}");
    }
}

/// Prints a gatekeeper event when `EEPVIEW_LOG` is set.
pub fn line(what: &str) {
    print(&text("gatekeeper", what));
}

/// Prints a first-load event of a tab webview when `EEPVIEW_LOG` is set.
pub fn load(what: &str) {
    print(&text("load", what));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_name_their_part() {
        assert_eq!(text("gatekeeper", "GET x"), "[eepview] gatekeeper GET x");
        assert_eq!(
            text("load", "tab-1-0 navigate u"),
            "[eepview] load tab-1-0 navigate u"
        );
        line("test line");
        load("test line");
    }
}
