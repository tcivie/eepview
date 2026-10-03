// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Router statistics from the text of a console page (`docs/wiki/router-console.md`,
//! R27 to R30). Each field is read alone and fails closed: a value that does not match its
//! rule exactly is `None`, never a guess.

use crate::net::stats::{Bandwidth, BuildSuccess, RouterStats, Tunnels};

const UPTIME_IDS: [&str; 3] = ["sb_general", "sb_shortgeneral", "sb_advancedgeneral"];

/// The Java I2P sidebar classes and the contract network status each one gives.
const NET_CLASSES: [(&str, &str); 8] = [
    ("running", "OK"),
    ("firewalled", "FIREWALLED"),
    ("testing", "TESTING"),
    ("hidden", "HIDDEN"),
    ("warn", "WARN"),
    ("error", "ERROR"),
    ("clockskew", "CLOCK_SKEW"),
    ("vmcomm", "VMCOMM"),
];

const I2PD_STATUS: [&str; 6] = ["OK", "Firewalled", "Unknown", "Proxy", "Mesh", "Stan"];

/// The i2pd uptime parts in their fixed order: singular, plural, milliseconds.
const I2PD_UNITS: [(&str, &str, u64); 4] = [
    ("day", "days", 86_400_000),
    ("hour", "hours", 3_600_000),
    ("minute", "minutes", 60_000),
    ("second", "seconds", 1_000),
];

// ------------------------------------------------------------------ numbers (R27)

/// One or more ASCII digits that fit in `u64`.
fn integer(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// A decimal times `scale`, exact on the decimal digits, a half rounds up.
fn scaled(text: &str, scale: u64) -> Option<u64> {
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    if text.contains('.') && fraction.is_empty() {
        return None;
    }
    let whole = integer(whole)?;
    let fraction = fraction.trim_end_matches('0');
    let numerator = if fraction.is_empty() {
        0
    } else {
        u128::from(integer(fraction)?)
    };
    let denominator = 10_u128.checked_pow(u32::try_from(fraction.len()).ok()?)?;
    let scale = u128::from(scale);
    let rounded = (2 * numerator * scale + denominator) / (2 * denominator);
    u64::try_from(u128::from(whole) * scale + rounded).ok()
}

// ------------------------------------------------------------------ Java I2P (R28, R29)

/// The Java I2P sidebar body (R27 to R29) to statistics.
#[must_use]
pub fn parse_java_summary(body: &str) -> RouterStats {
    let (uptime_ms, uptime_resolution_ms) = java_uptime(body).unzip();
    let (active_peers, floodfills, known_routers) = java_peers(body);
    let (exploratory, client, participating) = java_tunnels(body);
    RouterStats {
        uptime_ms,
        uptime_resolution_ms,
        network_status: java_network_status(body),
        active_peers,
        floodfills,
        known_routers,
        tunnels: Tunnels {
            participating,
            client,
            exploratory,
            ..Tunnels::default()
        },
        bandwidth_bytes_per_second: java_bandwidth(body),
        ..RouterStats::default()
    }
}

/// The text of the last cell of each row of the table `id`: `None` when the anchor is not
/// there exactly once, or the table or a row is cut.
fn table_values<'a>(body: &'a str, id: &str) -> Option<Vec<&'a str>> {
    if body.matches(&format!("id=\"{id}\"")).count() != 1 {
        return None;
    }
    let open = format!("<table id=\"{id}\">");
    let (_, rest) = body.split_once(&open)?;
    let (inner, _) = rest.split_once("</table>")?;
    let mut rows = inner.split("<tr");
    if !rows.next()?.trim().is_empty() {
        return None;
    }
    rows.map(row_value).collect()
}

/// The value of one `<tr…>…</tr>` row: the text of its last `<td…>` cell. Only white space
/// may follow the row, so a damaged row never moves the rows after it.
fn row_value(row: &str) -> Option<&str> {
    if !row.starts_with(|c: char| c == '>' || c.is_ascii_whitespace()) {
        return None;
    }
    let (row, after) = row.split_once("</tr>")?;
    if !after.trim().is_empty() {
        return None;
    }
    let cell = &row[row.rfind("<td")?..];
    let (_, cell) = cell.split_once('>')?;
    cell.split_once("</td>").map(|(value, _)| value)
}

fn java_uptime(body: &str) -> Option<(u64, u64)> {
    let id = UPTIME_IDS
        .into_iter()
        .find(|id| body.contains(&format!("id=\"{id}\"")))?;
    let rows = table_values(body, id)?;
    let at = match (id == "sb_advancedgeneral", rows.len()) {
        (false, 2) | (true, 4) => 1,
        (true, 5) => 2,
        _ => return None,
    };
    java_uptime_value(rows.get(at)?)
}

/// `N&nbsp;<unit>` with an English unit: the uptime and its resolution in ms.
fn java_uptime_value(value: &str) -> Option<(u64, u64)> {
    let (count, unit) = value.split_once("&nbsp;")?;
    let unit_ms = match unit {
        "ms" => 1,
        "sec" => 1_000,
        "min" => 60_000,
        "hour" | "hours" => 3_600_000,
        "day" | "days" => 86_400_000,
        _ => return None,
    };
    Some((integer(count)?.checked_mul(unit_ms)?, unit_ms))
}

fn java_bandwidth(body: &str) -> Bandwidth {
    let rows = table_values(body, "sb_bandwidth").filter(|r| (2..=4).contains(&r.len()));
    let Some(rows) = rows else {
        return Bandwidth::default();
    };
    let (now_in, now_out) = rows.first().and_then(|v| java_rate_pair(v)).unzip();
    let five = rows.get(1).filter(|_| rows.len() == 4);
    let (avg_in, avg_out) = five.and_then(|v| java_rate_pair(v)).unzip();
    Bandwidth {
        in1s: now_in,
        out1s: now_out,
        in5m: avg_in,
        out5m: avg_out,
    }
}

/// `A / B&nbsp;KBps` or `A / B&nbsp;MBps` in bytes per second.
fn java_rate_pair(value: &str) -> Option<(u64, u64)> {
    let (pair, unit) = value.split_once("&nbsp;")?;
    let scale = match unit {
        "KBps" => 1_000,
        "MBps" => 1_000_000,
        _ => return None,
    };
    let (a, b) = pair.split_once(" / ")?;
    Some((scaled(a, scale)?, scaled(b, scale)?))
}

/// Active peers, floodfills and known routers.
fn java_peers(body: &str) -> (Option<u64>, Option<u64>, Option<u64>) {
    let (id, count) = if body.contains("id=\"sb_peers\"") {
        ("sb_peers", 5)
    } else {
        ("sb_peersadvanced", 6)
    };
    let rows = table_values(body, id).filter(|r| r.len() == count);
    let Some(rows) = rows else {
        return (None, None, None);
    };
    let active = rows.first().and_then(|v| active_peers(v));
    let count_at = |at: usize| rows.get(at).and_then(|v| integer(v));
    (active, count_at(3), count_at(4))
}

/// `A / B`: the peers with a connection now.
fn active_peers(value: &str) -> Option<u64> {
    let (now, recent) = value.split_once(" / ")?;
    integer(recent)?;
    integer(now)
}

/// Exploratory, client and participating tunnels.
fn java_tunnels(body: &str) -> (Option<u64>, Option<u64>, Option<u64>) {
    let rows = table_values(body, "sb_tunnels").filter(|r| r.len() == 4);
    let Some(rows) = rows else {
        return (None, None, None);
    };
    let count_at = |at: usize| rows.get(at).and_then(|v| integer(v));
    (count_at(0), count_at(1), count_at(2))
}

/// The class of the first `<span class="sb_netstatus <class>">`.
fn java_network_status(body: &str) -> Option<String> {
    let (_, rest) = body.split_once("<span class=\"sb_netstatus ")?;
    let (class, tail) = rest.split_once('"')?;
    if !tail.starts_with('>') {
        return None;
    }
    NET_CLASSES
        .iter()
        .find(|(name, _)| *name == class)
        .map(|(_, status)| (*status).to_owned())
}

// ------------------------------------------------------------------ i2pd (R30)

/// The i2pd main page body (R27, R30) to statistics. English pages only.
#[must_use]
pub fn parse_i2pd_main(body: &str) -> RouterStats {
    if !body.contains("<html lang=\"en\"") {
        return RouterStats::default();
    }
    let count = |label: &str, end: &str| label_value(body, label, end).and_then(integer);
    let (uptime_ms, uptime_resolution_ms) = label_value(body, "Uptime", "<br>")
        .and_then(i2pd_uptime)
        .map(|ms| (ms, 1_000))
        .unzip();
    RouterStats {
        uptime_ms,
        uptime_resolution_ms,
        network_status: label_value(body, "Network status", "<br>").and_then(i2pd_status),
        known_routers: count("Routers", "&nbsp;"),
        floodfills: count("Floodfills", "&nbsp;"),
        tunnels: Tunnels {
            participating: count("Transit Tunnels", "<br>"),
            ..Tunnels::default()
        },
        bandwidth_bytes_per_second: Bandwidth {
            in1s: i2pd_rate(body, "Received"),
            out1s: i2pd_rate(body, "Sent"),
            ..Bandwidth::default()
        },
        tunnel_build_success_percent: BuildSuccess {
            total: label_value(body, "Tunnel creation success rate", "<br>")
                .and_then(|v| v.strip_suffix('%'))
                .and_then(integer),
            ..BuildSuccess::default()
        },
        ..RouterStats::default()
    }
}

/// The text after `<b><label>:</b> ` up to `end`. `None` when the label is not in the body
/// exactly once, `end` does not follow it, or the text holds a tag or a line end (the
/// terminator belongs to a later line).
fn label_value<'a>(body: &'a str, label: &str, end: &str) -> Option<&'a str> {
    let marker = format!("<b>{label}:</b> ");
    let mut parts = body.split(marker.as_str()).skip(1);
    let rest = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let (value, _) = rest.split_once(end)?;
    (!value.contains(['<', '\r', '\n'])).then_some(value)
}

fn i2pd_status(value: &str) -> Option<String> {
    I2PD_STATUS
        .contains(&value)
        .then(|| value.to_ascii_uppercase())
}

/// `D day(s), H hour(s), M minute(s), S second(s)`: seconds always, the order fixed.
fn i2pd_uptime(text: &str) -> Option<u64> {
    let mut next = 0;
    let mut total: u64 = 0;
    for part in text.split(", ") {
        let (count, unit) = part.split_once(' ')?;
        let (at, (_, _, unit_ms)) = I2PD_UNITS
            .iter()
            .enumerate()
            .find(|(_, (one, many, _))| unit == *one || unit == *many)?;
        if at < next {
            return None;
        }
        next = at + 1;
        total = total.checked_add(integer(count)?.checked_mul(*unit_ms)?)?;
    }
    (next == I2PD_UNITS.len()).then_some(total)
}

/// `<amount> (X KiB/s)` in bytes per second.
fn i2pd_rate(body: &str, label: &str) -> Option<u64> {
    let value = label_value(body, label, "<br>")?.strip_suffix(" KiB/s)")?;
    let (_, kib) = value.rsplit_once(" (")?;
    scaled(kib, 1_024)
}
