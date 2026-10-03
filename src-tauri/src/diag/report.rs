// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The bug report: its text (the preview and the file) and the prefilled GitHub issue URL.
//! Every value is scrubbed again here, and the URL always starts with [`ISSUE_PREFIX`].

use std::fmt::Write as _;

use super::scrub::scrub_report as scrub;
use super::sysinfo::SystemInfo;

/// The only URL eepview ever opens outside I2P.
pub const ISSUE_PREFIX: &str = "https://github.com/tcivie/eepview/issues/new";
/// The longest issue URL.
pub const MAX_URL: usize = 8000;
/// The most log lines in a report.
pub const PREVIEW_EVENTS: usize = 200;
/// The first log line when older lines were left out of the URL.
pub const TRIM_NOTE: &str = "[log trimmed, the full log is in the attached file]";

/// Where the user started the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportKind {
    /// The menu, or anything else.
    General,
    /// The crash banner.
    Crash,
    /// The blocked page.
    Blocked,
    /// The router-down page.
    RouterDown,
    /// A page that did not load.
    LoadFailed,
}

impl ReportKind {
    /// The kind of a `kind=` parameter.
    #[must_use]
    pub fn from_param(param: &str) -> Self {
        match param {
            "crash" => Self::Crash,
            "blocked" => Self::Blocked,
            "router-down" => Self::RouterDown,
            "load-failed" => Self::LoadFailed,
            _ => Self::General,
        }
    }

    /// The issue title.
    #[must_use]
    pub fn title(self) -> &'static str {
        [
            "Problem report",
            "Crash report",
            "Blocked page report",
            "Router down report",
            "Page load report",
        ][self as usize]
    }
}

/// Everything a report holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// Where it started.
    pub kind: ReportKind,
    /// The user's words.
    pub description: String,
    /// The system facts.
    pub info: SystemInfo,
    /// Formatted log lines, oldest first.
    pub log: Vec<String>,
    /// The user wants the log in it.
    pub include_log: bool,
}

fn description(report: &Report) -> String {
    let text = report.description.trim();
    if text.is_empty() {
        "(not given)".to_owned()
    } else {
        text.to_owned()
    }
}

/// The `System:` part and, when included, the log part with `log` as its lines.
fn diagnostics(report: &Report, log: &[String], trimmed: bool) -> String {
    let mut out = format!("System:\n{}\n", report.info.lines().join("\n"));
    if report.include_log {
        let _ = write!(
            out,
            "\nDiagnostics log (last {} events):\n",
            report.log.len()
        );
        if trimmed {
            out.push_str(TRIM_NOTE);
            out.push('\n');
        }
        for line in log {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// The report text: the preview and the file, scrubbed.
#[must_use]
pub fn text(report: &Report) -> String {
    scrub(&format!(
        "What happened:\n{}\n\n{}",
        description(report),
        diagnostics(report, &report.log, false)
    ))
}

/// `%XX` for every byte but `A-Z a-z 0-9 - _ . ~`.
#[must_use]
pub fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// The URL for a description and the diagnostics text.
fn url_of(report: &Report, what: &str, diag: &str) -> String {
    let info = &report.info;
    let pairs = [
        ("template", "bug.yml".to_owned()),
        ("labels", "bug".to_owned()),
        ("title", report.kind.title().to_owned()),
        ("version", format!("{} ({})", info.version, info.commit)),
        ("os", format!("{} ({})", info.os, info.arch)),
        ("what-happened", what.to_owned()),
        ("diagnostics", diag.to_owned()),
    ];
    let query: Vec<String> = pairs
        .iter()
        .map(|(k, v)| format!("{k}={}", percent_encode(&scrub(v))))
        .collect();
    format!("{ISSUE_PREFIX}?{}", query.join("&"))
}

/// The issue URL, and whether log lines were left out to keep it under [`MAX_URL`].
#[must_use]
pub fn issue_url(report: &Report) -> (String, bool) {
    let what = description(report);
    let full = url_of(report, &what, &diagnostics(report, &report.log, false));
    if full.len() <= MAX_URL || !report.include_log || report.log.is_empty() {
        return if full.len() <= MAX_URL {
            (full, false)
        } else {
            (cut_description(report, &what, false), false)
        };
    }
    let mut skip = first_fit(report, &what);
    while skip <= report.log.len() {
        let url = url_of(
            report,
            &what,
            &diagnostics(report, &report.log[skip..], true),
        );
        if url.len() <= MAX_URL {
            return (url, true);
        }
        skip += 1;
    }
    (cut_description(report, &what, true), true)
}

/// The fewest oldest log lines to leave out so the URL fits, from the encoded size of each
/// line. At least one line goes.
fn first_fit(report: &Report, what: &str) -> usize {
    let base = url_of(report, what, &diagnostics(report, &[], true)).len();
    let costs: Vec<usize> = report
        .log
        .iter()
        .map(|line| percent_encode(&scrub(&format!("{line}\n"))).len())
        .collect();
    let mut rest: usize = costs.iter().sum();
    for (skip, cost) in costs.iter().enumerate() {
        if skip > 0 && base + rest <= MAX_URL {
            return skip;
        }
        rest -= cost;
    }
    costs.len()
}

/// The URL with no log lines and as much of the description as fits.
fn cut_description(report: &Report, what: &str, trimmed: bool) -> String {
    let diag = diagnostics(report, &[], trimmed);
    let chars: Vec<char> = what.chars().collect();
    let (mut low, mut high) = (0, chars.len());
    while low < high {
        let mid = (low + high).div_ceil(2);
        let cut: String = chars[..mid].iter().collect();
        if url_of(report, &format!("{cut}…"), &diag).len() <= MAX_URL {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    let cut: String = chars[..low].iter().collect();
    url_of(report, &format!("{cut}…"), &diag)
}

/// True for [`ISSUE_PREFIX`] alone or with a query.
#[must_use]
pub fn is_issue_url(url: &str) -> bool {
    url.strip_prefix(ISSUE_PREFIX)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('?'))
}

/// `eepview-report.txt` for 0 and 1, `eepview-report (<n>).txt` from 2. No date (R13.2).
#[must_use]
pub fn file_name(n: u32) -> String {
    if n < 2 {
        "eepview-report.txt".to_owned()
    } else {
        format!("eepview-report ({n}).txt")
    }
}
