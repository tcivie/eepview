// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests for the router statistics read from the console
//! (`docs/wiki/router-console.md`, "Router statistics from the console"): the request
//! (R32, R33) and the parsers (R35 to R38), against the two fixtures of the spec.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use proptest::prelude::*;
use proptest::test_runner::{Config, TestCaseError, TestRunner};

use crate::net::console::{
    ConsoleKind, STATS_MAX_ANSWER, STATS_TIMEOUT, fetch_stats, parse_console_stats,
    parse_i2pd_main, parse_java_summary, probe, stats_path,
};
use crate::net::stats::{Bandwidth, BuildSuccess, RouterStats, Tunnels};
use crate::net::testing::FakeConsole;

const JAVA: &str =
    include_str!("../../../tests/fixtures/console/java-2.13.0-xhr1-summaryframe.txt");
const I2PD: &str = include_str!("../../../tests/fixtures/console/i2pd-2.58.0-main-synthetic.txt");
const JAVA_PATH: &str = "/xhr1.jsp?requestURI=/summaryframe";

// ------------------------------------------------------------ expected values

/// The values the spec table gives for the Java I2P fixture.
fn expected_java() -> RouterStats {
    RouterStats {
        uptime_ms: Some(28_800_000),
        uptime_resolution_ms: Some(3_600_000),
        network_status: Some("OK".to_owned()),
        bandwidth_bytes_per_second: Bandwidth {
            in1s: Some(53_910),
            out1s: Some(37_370),
            in5m: Some(37_830),
            out5m: Some(33_060),
        },
        active_peers: Some(1678),
        known_routers: Some(4905),
        floodfills: Some(1570),
        tunnels: Tunnels {
            inbound: None,
            out: None,
            participating: Some(398),
            client: Some(2),
            exploratory: Some(11),
        },
        ..RouterStats::default()
    }
}

/// The values the spec table gives for the i2pd fixture.
fn expected_i2pd() -> RouterStats {
    RouterStats {
        uptime_ms: Some(93_784_000),
        uptime_resolution_ms: Some(1_000),
        network_status: Some("OK".to_owned()),
        bandwidth_bytes_per_second: Bandwidth {
            in1s: Some(12_636),
            out1s: Some(5_806),
            in5m: None,
            out5m: None,
        },
        active_peers: None,
        known_routers: Some(3021),
        floodfills: Some(812),
        tunnels: Tunnels {
            inbound: None,
            out: None,
            participating: Some(157),
            client: None,
            exploratory: None,
        },
        tunnel_build_success_percent: BuildSuccess {
            exploratory: None,
            client: None,
            total: Some(42),
        },
        ..RouterStats::default()
    }
}

// ------------------------------------------------------------ field checks

type Fields = Vec<(&'static str, Option<String>)>;

fn num(value: Option<u64>) -> Option<String> {
    value.map(|n| n.to_string())
}

/// Every field of the statistics (but the history), by name.
fn fields(s: &RouterStats) -> Fields {
    vec![
        ("version", s.version.clone()),
        ("uptime_ms", num(s.uptime_ms)),
        ("uptime_resolution_ms", num(s.uptime_resolution_ms)),
        ("network_status", s.network_status.clone()),
        ("known_routers", num(s.known_routers)),
        ("floodfills", num(s.floodfills)),
        ("active_peers", num(s.active_peers)),
        ("tunnels.in", num(s.tunnels.inbound)),
        ("tunnels.out", num(s.tunnels.out)),
        ("tunnels.participating", num(s.tunnels.participating)),
        ("tunnels.client", num(s.tunnels.client)),
        ("tunnels.exploratory", num(s.tunnels.exploratory)),
        ("bw.in1s", num(s.bandwidth_bytes_per_second.in1s)),
        ("bw.out1s", num(s.bandwidth_bytes_per_second.out1s)),
        ("bw.in5m", num(s.bandwidth_bytes_per_second.in5m)),
        ("bw.out5m", num(s.bandwidth_bytes_per_second.out5m)),
        (
            "build.exploratory",
            num(s.tunnel_build_success_percent.exploratory),
        ),
        ("build.client", num(s.tunnel_build_success_percent.client)),
        ("build.total", num(s.tunnel_build_success_percent.total)),
    ]
}

const UPTIME: [&str; 2] = ["uptime_ms", "uptime_resolution_ms"];
const BW_NOW: [&str; 2] = ["bw.in1s", "bw.out1s"];
const BW_5M: [&str; 2] = ["bw.in5m", "bw.out5m"];
const PEERS: [&str; 3] = ["active_peers", "floodfills", "known_routers"];
const TUNNELS: [&str; 3] = [
    "tunnels.participating",
    "tunnels.client",
    "tunnels.exploratory",
];

fn cat(groups: &[&[&'static str]]) -> Vec<&'static str> {
    groups.iter().flat_map(|g| g.iter().copied()).collect()
}

/// R35, fail closed per field: the fields in `nulls` are null; the fields in `maybe` are
/// null or the original value; every other field keeps its original value from `base`.
/// The history of a parse is empty.
fn check(what: &str, got: &RouterStats, base: &RouterStats, nulls: &[&str], maybe: &[&str]) {
    assert!(got.history.is_empty(), "{what}: a parser gives no history");
    for ((name, got), (_, want)) in fields(got).into_iter().zip(fields(base)) {
        if nulls.contains(&name) {
            assert_eq!(got, None, "{what}: {name} must be null");
        } else if maybe.contains(&name) {
            assert!(
                got.is_none() || got == want,
                "{what}: {name} must be null or {want:?}, not {got:?}"
            );
        } else {
            assert_eq!(got, want, "{what}: {name} must keep its value");
        }
    }
}

// ------------------------------------------------------------ mutation helpers

/// `body` with `from` replaced by `to`; `from` must be in the fixture exactly once.
fn swap(body: &str, from: &str, to: &str) -> String {
    assert_eq!(
        body.matches(from).count(),
        1,
        "the fixture holds {from:?} once"
    );
    body.replacen(from, to, 1)
}

fn row(value: &str) -> String {
    format!("<tr><td align=\"left\"><b>Label:</b></td><td align=\"right\">{value}</td></tr>\n")
}

fn table(id: &str, values: &[&str]) -> String {
    let rows: String = values.iter().map(|v| row(v)).collect();
    format!("<table id=\"{id}\">\n{rows}</table>\n")
}

/// The byte range of the table `id` in `body`, tags included.
fn table_range(body: &str, id: &str) -> (usize, usize) {
    let start = body
        .find(&format!("<table id=\"{id}\">"))
        .unwrap_or_else(|| panic!("the fixture has no table {id}"));
    let close = body[start..].find("</table>").expect("a table ends");
    (start, start + close + "</table>".len())
}

fn replace_table(body: &str, id: &str, values: &[&str]) -> String {
    let (start, end) = table_range(body, id);
    format!("{}{}{}", &body[..start], table(id, values), &body[end..])
}

fn remove_table(body: &str, id: &str) -> String {
    let (start, end) = table_range(body, id);
    format!("{}{}", &body[..start], &body[end..])
}

const GENERAL: [&str; 2] = ["2.13.0-0", "8&nbsp;hours"];
const BANDWIDTH: [&str; 4] = [
    "53.91 / 37.37&nbsp;KBps",
    "37.83 / 33.06&nbsp;KBps",
    "44.08 / 40.68&nbsp;KBps",
    "1.34&#8239;GB / 1.24&#8239;GB",
];
const PEER_ROWS: [&str; 5] = ["1678 / 2191", "35", "150", "1570", "4905"];
const TUNNEL_ROWS: [&str; 4] = ["11", "2", "398", "12.06"];

/// The Java fixture with one table replaced by `values`.
fn java_with(id: &str, values: &[&str]) -> String {
    replace_table(JAVA, id, values)
}

fn crlf(body: &str) -> String {
    body.replace('\n', "\r\n")
}

// ------------------------------------------------------------ fixtures (R36 to R38)

#[test]
fn r36_the_java_fixture_gives_the_values_of_the_spec_table() {
    assert_eq!(parse_java_summary(JAVA), expected_java());
}

#[test]
fn r36_java_never_gives_the_version_the_in_out_tunnels_or_the_build_success() {
    // R36: always null from Java I2P.
    let got = parse_java_summary(JAVA);
    assert_eq!(got.version, None, "version");
    assert_eq!(got.tunnels.inbound, None, "tunnels.in");
    assert_eq!(got.tunnels.out, None, "tunnels.out");
    assert_eq!(got.tunnel_build_success_percent, BuildSuccess::default());
    assert!(got.history.is_empty());
}

#[test]
fn r38_the_i2pd_fixture_gives_the_values_of_the_spec_table() {
    assert_eq!(parse_i2pd_main(I2PD), expected_i2pd());
}

#[test]
fn r38_i2pd_never_gives_the_version_or_the_fields_the_main_page_lacks() {
    // R38: always null from i2pd.
    let got = parse_i2pd_main(I2PD);
    assert_eq!(got.version, None, "version");
    assert_eq!(got.active_peers, None, "activePeers");
    assert_eq!(got.tunnels.inbound, None, "tunnels.in");
    assert_eq!(got.tunnels.out, None, "tunnels.out");
    assert_eq!(got.tunnels.exploratory, None, "tunnels.exploratory");
    assert_eq!(got.bandwidth_bytes_per_second.in5m, None, "in5m");
    assert_eq!(got.bandwidth_bytes_per_second.out5m, None, "out5m");
    assert_eq!(got.tunnel_build_success_percent.exploratory, None);
    assert_eq!(got.tunnel_build_success_percent.client, None);
}

#[test]
fn r38_i2pd_fixture_gives_no_client_tunnel_count() {
    // R38: the `Client Tunnels` line of the fixture holds 14, and it is not read.
    assert_eq!(parse_i2pd_main(I2PD).tunnels.client, None, "tunnels.client");
}

#[test]
fn r38_crlf_line_ends_give_the_same_result_for_i2pd() {
    assert_eq!(parse_i2pd_main(&crlf(I2PD)), expected_i2pd());
}

#[test]
fn r38_crlf_line_ends_give_the_same_result_for_java() {
    assert_eq!(parse_java_summary(&crlf(JAVA)), expected_java());
}

#[test]
fn r38_a_mix_of_lf_and_crlf_line_ends_gives_the_same_result() {
    let mixed: String = I2PD
        .split_inclusive('\n')
        .enumerate()
        .map(|(i, line)| {
            if i % 2 == 0 {
                crlf(line)
            } else {
                line.to_owned()
            }
        })
        .collect();
    assert_eq!(parse_i2pd_main(&mixed), expected_i2pd());
}

#[test]
fn r32_parse_console_stats_runs_the_parser_of_the_kind() {
    assert_eq!(
        parse_console_stats(ConsoleKind::Java, JAVA),
        expected_java()
    );
    assert_eq!(
        parse_console_stats(ConsoleKind::I2pd, I2PD),
        expected_i2pd()
    );
}

#[test]
fn r35_a_page_of_the_other_router_gives_all_null() {
    // R35, R36, R38: no anchor, no field.
    assert_eq!(parse_java_summary(I2PD), RouterStats::default());
    assert_eq!(parse_i2pd_main(JAVA), RouterStats::default());
    assert_eq!(
        parse_console_stats(ConsoleKind::Java, I2PD),
        RouterStats::default()
    );
    assert_eq!(
        parse_console_stats(ConsoleKind::I2pd, JAVA),
        RouterStats::default()
    );
}

#[test]
fn r35_an_empty_body_gives_all_null() {
    for kind in [ConsoleKind::Java, ConsoleKind::I2pd] {
        assert_eq!(
            parse_console_stats(kind, ""),
            RouterStats::default(),
            "{kind:?}"
        );
        assert_eq!(
            parse_console_stats(kind, "\r\n"),
            RouterStats::default(),
            "{kind:?}"
        );
        assert_eq!(
            parse_console_stats(kind, "<html></html>"),
            RouterStats::default()
        );
    }
}

// ------------------------------------------------------------ Java: language (R36, R37)

#[test]
fn r36_java_reads_rows_by_position_in_every_console_language() {
    // R36: "never reads a row label or a title, so it works in every console language".
    let mut body = JAVA.replace(" title=\"", " data-t=\"");
    for label in [
        "Version:",
        "Uptime:",
        "3&nbsp;sec:",
        "5&nbsp;min:",
        "Total:",
        "Used:",
        "Active:",
        "Fast:",
        "High capacity:",
        "Floodfill:",
        "Known:",
        "Exploratory:",
        "Client:",
        "Participating:",
        "Share ratio:",
    ] {
        body = swap(&body, &format!("<b>{label}</b>"), "<b>Zzz</b>");
    }
    body = swap(&body, "Network: OK", "Netzwerk: in Ordnung");
    let got = parse_java_summary(&body);
    check("translated labels", &got, &expected_java(), &[], &[]);
}

#[test]
fn r37_java_uptime_with_a_translated_unit_is_null_and_the_rest_stays() {
    // R37: "Only the English units parse"; R36: the other figures work in every language.
    for unit in [
        "Stunden",
        "heures",
        "horas",
        "\u{43b}\u{43e}\u{434}",
        "\u{5c0f}\u{65f6}",
    ] {
        let body = swap(JAVA, "8&nbsp;hours", &format!("8&nbsp;{unit}"));
        let got = parse_java_summary(&body);
        check(unit, &got, &expected_java(), &UPTIME, &[]);
    }
}

#[test]
fn r37_java_uptime_units_convert_to_milliseconds() {
    // R37: unit table, and the resolution is the unit.
    let cases: [(&str, u64, u64); 10] = [
        ("500&nbsp;ms", 500, 1),
        ("1&nbsp;sec", 1_000, 1_000),
        ("45&nbsp;sec", 45_000, 1_000),
        ("7&nbsp;min", 420_000, 60_000),
        ("1&nbsp;hour", 3_600_000, 3_600_000),
        ("8&nbsp;hours", 28_800_000, 3_600_000),
        ("1&nbsp;day", 86_400_000, 86_400_000),
        ("3&nbsp;days", 259_200_000, 86_400_000),
        ("0&nbsp;sec", 0, 1_000),
        ("0&nbsp;days", 0, 86_400_000),
    ];
    for (value, ms, resolution) in cases {
        let got = parse_java_summary(&java_with("sb_general", &["2.13.0-0", value]));
        let base = RouterStats {
            uptime_ms: Some(ms),
            uptime_resolution_ms: Some(resolution),
            ..expected_java()
        };
        check(value, &got, &base, &[], &[]);
    }
}

#[test]
fn r37_java_uptime_that_is_not_n_nbsp_unit_is_null() {
    // R35, R37: an integer, `&nbsp;`, an English unit; nothing else.
    for value in [
        "8&nbsp;years",
        "8&nbsp;year",
        "8&nbsp;h",
        "8&nbsp;hrs",
        "8&nbsp;weeks",
        "8&nbsp;",
        "8 hours",
        "8.5&nbsp;hours",
        "-8&nbsp;hours",
        "+8&nbsp;hours",
        "8,5&nbsp;hours",
        "\u{ff11}\u{ff12}&nbsp;hours",
        "&nbsp;hours",
        "hours",
        "",
        "8&nbsp;hours&nbsp;ago",
        "99999999999999999999&nbsp;days",
        "18446744073709551615&nbsp;days",
    ] {
        let got = parse_java_summary(&java_with("sb_general", &["2.13.0-0", value]));
        check(value, &got, &expected_java(), &UPTIME, &[]);
    }
}

#[test]
fn r36_java_uptime_is_row_1_of_a_table_with_2_rows() {
    // R36: sb_general or sb_shortgeneral with 2 rows: row 1.
    for id in ["sb_general", "sb_shortgeneral"] {
        let got = parse_java_summary(&table(id, &["5&nbsp;sec", "2&nbsp;min"]));
        let want = RouterStats {
            uptime_ms: Some(120_000),
            uptime_resolution_ms: Some(60_000),
            ..RouterStats::default()
        };
        assert_eq!(got, want, "{id}");
    }
}

#[test]
fn r36_java_advanced_uptime_is_row_1_of_4_rows_and_row_2_of_5_rows() {
    let four = ["5&nbsp;sec", "2&nbsp;min", "3&nbsp;hours", "4&nbsp;days"];
    let five = [
        "5&nbsp;sec",
        "6&nbsp;sec",
        "2&nbsp;min",
        "3&nbsp;hours",
        "4&nbsp;days",
    ];
    let want = RouterStats {
        uptime_ms: Some(120_000),
        uptime_resolution_ms: Some(60_000),
        ..RouterStats::default()
    };
    assert_eq!(
        parse_java_summary(&table("sb_advancedgeneral", &four)),
        want
    );
    assert_eq!(
        parse_java_summary(&table("sb_advancedgeneral", &five)),
        want
    );
}

#[test]
fn r36_java_uptime_with_another_row_count_is_null() {
    // R36: "When a table has another number of rows than the rule names, every field of
    // that table is null."
    let rows = [
        "1&nbsp;sec",
        "2&nbsp;min",
        "3&nbsp;hours",
        "4&nbsp;days",
        "5&nbsp;sec",
    ];
    let wrong: [(&str, [usize; 5]); 3] = [
        ("sb_general", [0, 1, 3, 4, 5]),
        ("sb_shortgeneral", [0, 1, 3, 4, 5]),
        ("sb_advancedgeneral", [0, 1, 2, 3, 6]),
    ];
    for (id, counts) in wrong {
        for count in counts {
            let values: Vec<&str> = (0..count).map(|i| rows[i % rows.len()]).collect();
            let got = parse_java_summary(&table(id, &values));
            assert_eq!(got, RouterStats::default(), "{id} with {count} rows");
        }
    }
}

#[test]
fn r36_java_uptime_table_ids_win_in_the_order_general_short_advanced() {
    // R36: "The first of the three ids in this order wins."
    let general = table("sb_general", &["x", "2&nbsp;min"]);
    let short = table("sb_shortgeneral", &["x", "1&nbsp;sec"]);
    let advanced = table("sb_advancedgeneral", &["x", "9&nbsp;days", "x", "x"]);
    let two_minutes = RouterStats {
        uptime_ms: Some(120_000),
        uptime_resolution_ms: Some(60_000),
        ..RouterStats::default()
    };
    for body in [
        format!("{short}{general}"),
        format!("{general}{short}"),
        format!("{advanced}{short}{general}"),
        format!("{general}{advanced}"),
    ] {
        assert_eq!(parse_java_summary(&body), two_minutes, "{body}");
    }
    let one_second = RouterStats {
        uptime_ms: Some(1_000),
        uptime_resolution_ms: Some(1_000),
        ..RouterStats::default()
    };
    assert_eq!(
        parse_java_summary(&format!("{advanced}{short}")),
        one_second
    );
}

// ------------------------------------------------------------ Java: bandwidth (R36)

#[test]
fn r36_java_bandwidth_rows_are_read_by_position() {
    // R36: row 0 is now, row 1 is 5 min; units K (x 1 000) and M (x 1 000 000).
    let rows = [
        "1 / 2&nbsp;KBps",
        "3 / 4&nbsp;KBps",
        "5 / 6&nbsp;KBps",
        "7 / 8&nbsp;KBps",
    ];
    let got = parse_java_summary(&java_with("sb_bandwidth", &rows));
    let bandwidth = Bandwidth {
        in1s: Some(1_000),
        out1s: Some(2_000),
        in5m: Some(3_000),
        out5m: Some(4_000),
    };
    let base = RouterStats {
        bandwidth_bytes_per_second: bandwidth,
        ..expected_java()
    };
    check("rows by position", &got, &base, &[], &[]);
}

#[test]
fn r36_java_bandwidth_in_mbps_is_multiplied_by_a_million() {
    let rows = ["1.5 / 0.25&nbsp;MBps", "2 / 0.5&nbsp;MBps", "x", "x"];
    let got = parse_java_summary(&java_with("sb_bandwidth", &rows));
    let bandwidth = Bandwidth {
        in1s: Some(1_500_000),
        out1s: Some(250_000),
        in5m: Some(2_000_000),
        out5m: Some(500_000),
    };
    let base = RouterStats {
        bandwidth_bytes_per_second: bandwidth,
        ..expected_java()
    };
    check("MBps", &got, &base, &[], &[]);
}

#[test]
fn r36_java_bandwidth_of_a_young_router_has_no_5_minute_figure() {
    // R36: with 2 or 3 rows, row 1 is not read: the router is younger than 6 minutes.
    for rows in [&BANDWIDTH[..2], &BANDWIDTH[..3]] {
        let got = parse_java_summary(&java_with("sb_bandwidth", rows));
        check("2 or 3 rows", &got, &expected_java(), &BW_5M, &[]);
    }
}

#[test]
fn r36_java_bandwidth_with_any_other_row_count_is_null() {
    // R36: "any row count from 2 to 4"; another count nulls the whole table.
    let five = ["1 / 2&nbsp;KBps"; 5];
    for rows in [&BANDWIDTH[..0], &BANDWIDTH[..1], &five[..]] {
        let got = parse_java_summary(&java_with("sb_bandwidth", rows));
        let all = cat(&[&BW_NOW, &BW_5M]);
        check(
            &format!("{} rows", rows.len()),
            &got,
            &expected_java(),
            &all,
            &[],
        );
    }
}

#[test]
fn r36_java_bandwidth_with_a_bad_unit_or_form_is_null_for_the_row() {
    // R35, R36: `A / B&nbsp;KBps` exactly; the unit is K or M with `Bps`.
    for value in [
        "53.91 / 37.37&nbsp;KiBps",
        "53.91 / 37.37&nbsp;kBps",
        "53.91 / 37.37&nbsp;GBps",
        "53.91 / 37.37&nbsp;Bps",
        "53.91 / 37.37&nbsp;KB/s",
        "53.91 / 37.37 KBps",
        "53.91 / 37.37",
        "53.91/37.37&nbsp;KBps",
        "53.91 /37.37&nbsp;KBps",
        "53.91  / 37.37&nbsp;KBps",
        "53.91 / 37.37&nbsp;\u{41a}Bps",
        "",
        "abc",
    ] {
        let rows = [value, BANDWIDTH[1], BANDWIDTH[2], BANDWIDTH[3]];
        let got = parse_java_summary(&java_with("sb_bandwidth", &rows));
        check(value, &got, &expected_java(), &BW_NOW, &[]);
    }
}

// R35: digits, then optionally `.` and digits. No sign, no group separator, no `,`.
const NOT_A_DECIMAL: [&str; 16] = [
    "5e3",
    "+5",
    "-5",
    ".5",
    "5.",
    "1 234",
    "1,5",
    "1.234,5",
    "\u{ff11}",
    "1.2.3",
    "abc",
    "0x10",
    "99999999999999999999",
    "18446744073709552",
    "NaN",
    "inf",
];

fn check_bad_bandwidth_first_value(bad: &str) {
    let first = [format!("{bad} / 37.37&nbsp;KBps"), BANDWIDTH[1].to_owned()];
    let rows = [
        first[0].as_str(),
        first[1].as_str(),
        BANDWIDTH[2],
        BANDWIDTH[3],
    ];
    let got = parse_java_summary(&java_with("sb_bandwidth", &rows));
    check(
        &format!("A = {bad}"),
        &got,
        &expected_java(),
        &["bw.in1s"],
        &["bw.out1s"],
    );
}

fn check_bad_bandwidth_second_value(bad: &str) {
    let second = [format!("53.91 / {bad}&nbsp;KBps"), BANDWIDTH[1].to_owned()];
    let rows = [
        second[0].as_str(),
        second[1].as_str(),
        BANDWIDTH[2],
        BANDWIDTH[3],
    ];
    let got = parse_java_summary(&java_with("sb_bandwidth", &rows));
    check(
        &format!("B = {bad}"),
        &got,
        &expected_java(),
        &["bw.out1s"],
        &["bw.in1s"],
    );
}

#[test]
fn r35_java_bandwidth_value_that_is_not_a_decimal_is_null_for_that_field() {
    for bad in NOT_A_DECIMAL {
        check_bad_bandwidth_first_value(bad);
        check_bad_bandwidth_second_value(bad);
    }
}

#[test]
fn r35_java_a_bad_5_minute_row_leaves_the_now_row_alone() {
    let rows = [
        BANDWIDTH[0],
        "5e3 / 33.06&nbsp;KBps",
        BANDWIDTH[2],
        BANDWIDTH[3],
    ];
    let got = parse_java_summary(&java_with("sb_bandwidth", &rows));
    check(
        "bad 5 min",
        &got,
        &expected_java(),
        &["bw.in5m"],
        &["bw.out5m"],
    );
}

#[test]
fn r35_java_conversion_is_exact_on_the_decimal_digits_and_a_half_rounds_up() {
    // R35: 0.5 -> 1, 1.5 -> 2, 1000.5 -> 1001, 2001.5 -> 2002 (a float gives 1000, 2001).
    let cases: [(&str, u64, u64); 6] = [
        ("0.0005 / 0.0004&nbsp;KBps", 1, 0),
        ("0.0015 / 0.0014&nbsp;KBps", 2, 1),
        ("1.0005 / 2.0015&nbsp;KBps", 1_001, 2_002),
        ("0 / 0&nbsp;KBps", 0, 0),
        ("0.0000005 / 0.0000015&nbsp;MBps", 1, 2),
        ("18446744073709551.615 / 1&nbsp;KBps", u64::MAX, 1_000),
    ];
    for (value, inbound, out) in cases {
        let rows = [value, BANDWIDTH[1], BANDWIDTH[2], BANDWIDTH[3]];
        let got = parse_java_summary(&java_with("sb_bandwidth", &rows));
        let mut base = expected_java();
        base.bandwidth_bytes_per_second.in1s = Some(inbound);
        base.bandwidth_bytes_per_second.out1s = Some(out);
        check(value, &got, &base, &[], &[]);
    }
}

// ------------------------------------------------------------ Java: peers (R36)

#[test]
fn r36_java_peers_with_5_rows_read_rows_0_3_and_4() {
    let rows = ["10 / 20", "1", "2", "30", "40"];
    let got = parse_java_summary(&java_with("sb_peers", &rows));
    let base = RouterStats {
        active_peers: Some(10),
        floodfills: Some(30),
        known_routers: Some(40),
        ..expected_java()
    };
    check("5 rows", &got, &base, &[], &[]);
}

#[test]
fn r36_java_peersadvanced_with_6_rows_reads_rows_0_3_and_4() {
    let rows = ["10 / 20", "1", "2", "30", "40", "99"];
    let body = format!(
        "{}{}",
        remove_table(JAVA, "sb_peers"),
        table("sb_peersadvanced", &rows)
    );
    let got = parse_java_summary(&body);
    let base = RouterStats {
        active_peers: Some(10),
        floodfills: Some(30),
        known_routers: Some(40),
        ..expected_java()
    };
    check("sb_peersadvanced", &got, &base, &[], &[]);
}

#[test]
fn r36_java_peers_with_the_wrong_row_count_for_the_id_are_null() {
    // R36: sb_peers needs 5 rows, sb_peersadvanced needs 6.
    let six = ["10 / 20", "1", "2", "30", "40", "99"];
    for rows in [&PEER_ROWS[..0], &PEER_ROWS[..4], &six[..]] {
        let got = parse_java_summary(&java_with("sb_peers", rows));
        check(
            &format!("sb_peers, {} rows", rows.len()),
            &got,
            &expected_java(),
            &PEERS,
            &[],
        );
    }
    for rows in [&PEER_ROWS[..], &six[..4]] {
        let body = format!(
            "{}{}",
            remove_table(JAVA, "sb_peers"),
            table("sb_peersadvanced", rows)
        );
        let got = parse_java_summary(&body);
        let what = format!("sb_peersadvanced, {} rows", rows.len());
        check(&what, &got, &expected_java(), &PEERS, &[]);
    }
}

#[test]
fn r35_java_active_peers_that_is_not_a_slash_pair_is_null() {
    // R36: `A / B`: A, a space, `/`, a space, B.
    for value in [
        "1678",
        "1678/2191",
        "1678 /2191",
        "1678/ 2191",
        "a / b",
        "-1 / 5",
        "+1 / 5",
        "1,678 / 2,191",
        "\u{ff11}\u{ff16} / 5",
        "1.5 / 2",
        "99999999999999999999 / 5",
        "",
    ] {
        let rows = [
            value,
            PEER_ROWS[1],
            PEER_ROWS[2],
            PEER_ROWS[3],
            PEER_ROWS[4],
        ];
        let got = parse_java_summary(&java_with("sb_peers", &rows));
        check(value, &got, &expected_java(), &["active_peers"], &[]);
    }
}

#[test]
fn r35_java_floodfills_and_known_routers_that_are_not_integers_are_null() {
    for bad in [
        "15,70",
        "1570.0",
        "+1570",
        "\u{ff11}\u{ff15}",
        "-1570",
        "1 570",
        "",
        "abc",
        "1570x",
        "99999999999999999999",
    ] {
        let body = swap(JAVA, ">1570<", &format!(">{bad}<"));
        let got = parse_java_summary(&body);
        check(
            &format!("floodfills {bad}"),
            &got,
            &expected_java(),
            &["floodfills"],
            &[],
        );
        let body = swap(JAVA, ">4905<", &format!(">{bad}<"));
        let got = parse_java_summary(&body);
        check(
            &format!("known {bad}"),
            &got,
            &expected_java(),
            &["known_routers"],
            &[],
        );
    }
}

// ------------------------------------------------------------ Java: tunnels (R36)

#[test]
fn r36_java_tunnel_rows_are_exploratory_client_participating() {
    let got = parse_java_summary(&java_with("sb_tunnels", &["1", "2", "3", "4"]));
    let mut base = expected_java();
    base.tunnels.exploratory = Some(1);
    base.tunnels.client = Some(2);
    base.tunnels.participating = Some(3);
    check("positions", &got, &base, &[], &[]);
}

#[test]
fn r36_java_tunnels_with_another_row_count_are_null() {
    let five = ["1", "2", "3", "4", "5"];
    for rows in [&TUNNEL_ROWS[..0], &TUNNEL_ROWS[..3], &five[..]] {
        let got = parse_java_summary(&java_with("sb_tunnels", rows));
        check(
            &format!("{} rows", rows.len()),
            &got,
            &expected_java(),
            &TUNNELS,
            &[],
        );
    }
}

#[test]
fn r35_java_tunnel_counts_that_are_not_integers_are_null_one_by_one() {
    for bad in [
        "1.0",
        "1,1",
        "-1",
        "+1",
        "\u{ff11}",
        "",
        "x",
        "99999999999999999999",
    ] {
        for (from, field) in [
            (">11<", "tunnels.exploratory"),
            (">2<", "tunnels.client"),
            (">398<", "tunnels.participating"),
        ] {
            let got = parse_java_summary(&swap(JAVA, from, &format!(">{bad}<")));
            check(
                &format!("{field} = {bad}"),
                &got,
                &expected_java(),
                &[field],
                &[],
            );
        }
    }
}

#[test]
fn r35_java_a_zero_count_is_a_number_not_a_missing_one() {
    let got = parse_java_summary(&swap(JAVA, ">398<", ">0<"));
    assert_eq!(got.tunnels.participating, Some(0));
}

// ------------------------------------------------------------ Java: network status (R36)

#[test]
fn r36_java_network_status_classes_map_to_the_contract_names() {
    for (class, name) in [
        ("running", "OK"),
        ("firewalled", "FIREWALLED"),
        ("testing", "TESTING"),
        ("hidden", "HIDDEN"),
        ("warn", "WARN"),
        ("error", "ERROR"),
        ("clockskew", "CLOCK_SKEW"),
        ("vmcomm", "VMCOMM"),
    ] {
        let from = "<span class=\"sb_netstatus running\">";
        let body = swap(
            JAVA,
            from,
            &format!("<span class=\"sb_netstatus {class}\">"),
        );
        let got = parse_java_summary(&body);
        let base = RouterStats {
            network_status: Some(name.to_owned()),
            ..expected_java()
        };
        check(class, &got, &base, &[], &[]);
    }
}

#[test]
fn r36_java_network_status_with_another_class_is_null() {
    for class in ["", "foo", "ok", "running2", "network"] {
        let from = "<span class=\"sb_netstatus running\">";
        let body = swap(
            JAVA,
            from,
            &format!("<span class=\"sb_netstatus {class}\">"),
        );
        let got = parse_java_summary(&body);
        check(class, &got, &expected_java(), &["network_status"], &[]);
    }
}

#[test]
fn r36_java_network_status_reads_the_first_span_only() {
    let first = "<span class=\"sb_netstatus firewalled\">x</span>\n";
    let got = parse_java_summary(&format!("{first}{JAVA}"));
    assert_eq!(got.network_status.as_deref(), Some("FIREWALLED"));
    let bad = "<span class=\"sb_netstatus foo\">x</span>\n";
    let got = parse_java_summary(&format!("{bad}{JAVA}"));
    assert_eq!(
        got.network_status, None,
        "an unknown first class is null, not the next span"
    );
}

#[test]
fn r36_java_network_status_text_is_not_read() {
    // R36: the class decides, in every language; the text next to it does not.
    let body = swap(JAVA, "Network: OK", "Network: Firewalled");
    check(
        "text",
        &parse_java_summary(&body),
        &expected_java(),
        &[],
        &[],
    );
}

// ------------------------------------------------------------ Java: missing and doubled

#[test]
fn r35_java_a_missing_section_nulls_its_fields_only() {
    // R35: "A missing section gives null for its fields only."
    let cases: [(&str, Vec<&str>); 4] = [
        ("sb_general", UPTIME.to_vec()),
        ("sb_bandwidth", cat(&[&BW_NOW, &BW_5M])),
        ("sb_peers", PEERS.to_vec()),
        ("sb_tunnels", TUNNELS.to_vec()),
    ];
    for (id, nulls) in cases {
        let got = parse_java_summary(&remove_table(JAVA, id));
        check(&format!("no {id}"), &got, &expected_java(), &nulls, &[]);
    }
    let body = swap(
        JAVA,
        "<span class=\"sb_netstatus running\">",
        "<span class=\"x\">",
    );
    let got = parse_java_summary(&body);
    check(
        "no netstatus",
        &got,
        &expected_java(),
        &["network_status"],
        &[],
    );
}

#[test]
fn r35_java_an_anchor_that_is_in_the_body_twice_nulls_its_fields() {
    // R35: "When an anchor ... is in the body more than once, the fields it gives are null."
    let cases: [(&str, &[&str], Vec<&str>); 4] = [
        ("sb_general", &GENERAL, UPTIME.to_vec()),
        ("sb_bandwidth", &BANDWIDTH, cat(&[&BW_NOW, &BW_5M])),
        ("sb_peers", &PEER_ROWS, PEERS.to_vec()),
        ("sb_tunnels", &TUNNEL_ROWS, TUNNELS.to_vec()),
    ];
    for (id, rows, nulls) in cases {
        let doubled = format!("{JAVA}{}", table(id, rows));
        let got = parse_java_summary(&doubled);
        check(&format!("{id} twice"), &got, &expected_java(), &nulls, &[]);
        let different = format!("{JAVA}{}", table(id, &["9&nbsp;sec"; 4]));
        let got = parse_java_summary(&different);
        check(
            &format!("{id} twice, other rows"),
            &got,
            &expected_java(),
            &nulls,
            &[],
        );
    }
}

#[test]
fn r35_java_a_value_cut_at_the_end_of_the_body_is_null() {
    // R35, R33: a cut value has no terminator; it is null, never a shorter number.
    let end = JAVA.find(">398<").unwrap() + ">39".len();
    let got = parse_java_summary(&JAVA[..end]);
    let tunnels = ["tunnels.client", "tunnels.exploratory"];
    check(
        "cut in 398",
        &got,
        &expected_java(),
        &["tunnels.participating"],
        &tunnels,
    );
    let end = JAVA.find("8&nbsp;hours").unwrap() + "8&nbsp;hours".len();
    assert_eq!(parse_java_summary(&JAVA[..end]), RouterStats::default());
}

// ------------------------------------------------------------ i2pd (R38)

fn i2pd_with(from: &str, to: &str) -> RouterStats {
    parse_i2pd_main(&swap(I2PD, from, to))
}

const UPTIME_TEXT: &str = "1 day, 2 hours, 3 minutes, 4 seconds";

#[test]
fn r38_i2pd_uptime_forms_convert_to_milliseconds() {
    // R38: days, hours and minutes are optional, seconds always there, the order fixed;
    // singular or plural; the resolution is 1 000.
    let cases: [(&str, u64); 9] = [
        ("4 seconds", 4_000),
        ("1 second", 1_000),
        ("0 seconds", 0),
        ("3 minutes, 4 seconds", 184_000),
        ("1 minute, 1 second", 61_000),
        ("2 hours, 4 seconds", 7_204_000),
        ("1 day, 4 seconds", 86_404_000),
        ("2 days, 1 hour, 1 minute, 1 second", 176_461_000),
        ("10 days, 3 hours, 5 seconds", 874_805_000),
    ];
    for (value, ms) in cases {
        let got = i2pd_with(UPTIME_TEXT, value);
        let base = RouterStats {
            uptime_ms: Some(ms),
            ..expected_i2pd()
        };
        check(value, &got, &base, &[], &[]);
    }
}

#[test]
fn r38_i2pd_uptime_that_does_not_match_the_form_is_null_for_both_fields() {
    for value in [
        "4 seconds, 3 minutes",
        "1 day, 2 hours",
        "3 minutes",
        "1 day 2 hours, 4 seconds",
        "1 week, 4 seconds",
        "1.5 days, 4 seconds",
        "-1 days, 4 seconds",
        "1 day,  4 seconds",
        "1 day,4 seconds",
        "1 day, 4 seconds extra",
        "2 hours, 1 day, 4 seconds",
        "1 day, 1 day, 4 seconds",
        "4 sec",
        "four seconds",
        "",
        "18446744073709551615 days, 1 second",
        "1 day, 2 hours, 3 minutes, 4 seconds, 5 ms",
    ] {
        let got = i2pd_with(UPTIME_TEXT, value);
        check(value, &got, &expected_i2pd(), &UPTIME, &[]);
    }
}

#[test]
fn r38_i2pd_uptime_needs_its_br_terminator() {
    for end in ["</div>", "<br/>", "\n", " <br>"] {
        let body = swap(I2PD, "4 seconds<br>", &format!("4 seconds{end}"));
        let got = parse_i2pd_main(&body);
        check(end, &got, &expected_i2pd(), &UPTIME, &[]);
    }
}

#[test]
fn r38_i2pd_network_status_words_are_given_in_upper_case() {
    for (word, name) in [
        ("OK", "OK"),
        ("Firewalled", "FIREWALLED"),
        ("Unknown", "UNKNOWN"),
        ("Proxy", "PROXY"),
        ("Mesh", "MESH"),
        ("Stan", "STAN"),
    ] {
        let got = i2pd_with(
            "<b>Network status:</b> OK<br>",
            &format!("<b>Network status:</b> {word}<br>"),
        );
        let base = RouterStats {
            network_status: Some(name.to_owned()),
            ..expected_i2pd()
        };
        check(word, &got, &base, &[], &[]);
    }
}

#[test]
fn r38_i2pd_network_status_with_a_suffix_or_another_word_is_null() {
    for word in [
        "OK (Testing)",
        "Firewalled (Testing)",
        "Firewalled - Symmetric NAT",
        "OK - Clock skew",
        "Testing",
        "ok",
        "Okay",
        "OK ",
        " OK",
        "",
        "Error",
        "Unknown ",
    ] {
        let got = i2pd_with(
            "<b>Network status:</b> OK<br>",
            &format!("<b>Network status:</b> {word}<br>"),
        );
        check(word, &got, &expected_i2pd(), &["network_status"], &[]);
    }
    let got = i2pd_with(
        "<b>Network status:</b> OK<br>",
        "<b>Network status:</b> OK</div>",
    );
    check("no br", &got, &expected_i2pd(), &["network_status"], &[]);
}

#[test]
fn r38_i2pd_network_status_v6_is_a_different_label() {
    // R38: `Network status` (not `Network status v6`).
    let only_v6 = i2pd_with(
        "<b>Network status:</b> OK<br>",
        "<b>Network status v6:</b> OK<br>",
    );
    check(
        "only v6",
        &only_v6,
        &expected_i2pd(),
        &["network_status"],
        &[],
    );
    let both = "<b>Network status:</b> Firewalled<br>\n<b>Network status v6:</b> OK<br>";
    let got = i2pd_with("<b>Network status:</b> OK<br>", both);
    let base = RouterStats {
        network_status: Some("FIREWALLED".to_owned()),
        ..expected_i2pd()
    };
    check("v4 first", &got, &base, &[], &[]);
    let both = "<b>Network status v6:</b> Firewalled<br>\n<b>Network status:</b> OK<br>";
    let got = i2pd_with("<b>Network status:</b> OK<br>", both);
    check("v6 first", &got, &expected_i2pd(), &[], &[]);
}

#[test]
fn r38_i2pd_tunnel_creation_success_rate_is_an_integer_percent() {
    for (value, percent) in [("0%", 0), ("100%", 100), ("7%", 7)] {
        let from = "<b>Tunnel creation success rate:</b> 42%<br>";
        let got = i2pd_with(
            from,
            &format!("<b>Tunnel creation success rate:</b> {value}<br>"),
        );
        let mut base = expected_i2pd();
        base.tunnel_build_success_percent.total = Some(percent);
        check(value, &got, &base, &[], &[]);
    }
    for value in [
        "42.5%",
        "-42%",
        "+42%",
        "4 2%",
        "42 %",
        "42",
        "%",
        "abc%",
        "42%%",
        "\u{ff14}\u{ff12}%",
        "99999999999999999999%",
    ] {
        let from = "<b>Tunnel creation success rate:</b> 42%<br>";
        let got = i2pd_with(
            from,
            &format!("<b>Tunnel creation success rate:</b> {value}<br>"),
        );
        check(value, &got, &expected_i2pd(), &["build.total"], &[]);
    }
    let got = i2pd_with("42%<br>", "42%</div>");
    check("no br", &got, &expected_i2pd(), &["build.total"], &[]);
}

#[test]
fn r38_i2pd_total_tunnel_creation_success_rate_is_not_read() {
    // R38: "Total tunnel creation success rate" is a different label.
    let from = "<b>Tunnel creation success rate:</b> 42%<br>";
    let to = "<b>Total tunnel creation success rate:</b> 42%<br>";
    check(
        "other label",
        &i2pd_with(from, to),
        &expected_i2pd(),
        &["build.total"],
        &[],
    );
    let extra = format!("<b>Total tunnel creation success rate:</b> 99%<br>\n{from}");
    check("both", &i2pd_with(from, &extra), &expected_i2pd(), &[], &[]);
}

#[test]
fn r38_i2pd_bandwidth_converts_kib_per_second_exactly() {
    // R35, R38: X x 1 024, exact on the decimal digits, a half rounds up.
    let cases: [(&str, u64); 8] = [
        ("(12.34 KiB/s)", 12_636),
        ("(0 KiB/s)", 0),
        ("(1 KiB/s)", 1_024),
        ("(1.5 KiB/s)", 1_536),
        ("(0.0004 KiB/s)", 0),
        ("(0.0005 KiB/s)", 1),
        ("(0.00048828125 KiB/s)", 1),
        ("(0.00146484375 KiB/s)", 2),
    ];
    for (value, bytes) in cases {
        let got = i2pd_with("(12.34 KiB/s)<br>", &format!("{value}<br>"));
        let mut base = expected_i2pd();
        base.bandwidth_bytes_per_second.in1s = Some(bytes);
        check(value, &got, &base, &[], &[]);
        let got = i2pd_with("(5.67 KiB/s)<br>", &format!("{value}<br>"));
        let mut base = expected_i2pd();
        base.bandwidth_bytes_per_second.out1s = Some(bytes);
        check(value, &got, &base, &[], &[]);
    }
}

#[test]
fn r38_i2pd_bandwidth_that_does_not_match_its_form_is_null_for_that_field() {
    for value in [
        "(12,34 KiB/s)",
        "(12.34 KB/s)",
        "(12.34 MiB/s)",
        "(12.34 KiB/sec)",
        "(abc KiB/s)",
        "(-12.34 KiB/s)",
        "(+12.34 KiB/s)",
        "(12.34KiB/s)",
        "( 12.34 KiB/s)",
        "12.34 KiB/s",
        "(12.34 KiB/s",
        "(12.34 KiB/s) ",
        "(.5 KiB/s)",
        "(5. KiB/s)",
        "(1e3 KiB/s)",
        "(\u{ff11} KiB/s)",
        "()",
        "(99999999999999999999 KiB/s)",
        "(18014398509481984 KiB/s)",
    ] {
        let got = i2pd_with("(12.34 KiB/s)<br>", &format!("{value}<br>"));
        check(value, &got, &expected_i2pd(), &["bw.in1s"], &[]);
        let got = i2pd_with("(5.67 KiB/s)<br>", &format!("{value}<br>"));
        check(value, &got, &expected_i2pd(), &["bw.out1s"], &[]);
    }
    let got = i2pd_with("(12.34 KiB/s)<br>", "(12.34 KiB/s)</div>");
    check("received, no br", &got, &expected_i2pd(), &["bw.in1s"], &[]);
    let got = i2pd_with("(5.67 KiB/s)<br>", "(5.67 KiB/s)</div>");
    check("sent, no br", &got, &expected_i2pd(), &["bw.out1s"], &[]);
}

// R35, R38: Routers and Floodfills end with `&nbsp;`, Transit with `<br>`.
const I2PD_COUNTS: [(&str, &str, &str, &str); 3] = [
    (
        "<b>Routers:</b> 3021",
        "&nbsp;",
        "<b>Routers:</b> {}",
        "known_routers",
    ),
    (
        "<b>Floodfills:</b> 812",
        "&nbsp;",
        "<b>Floodfills:</b> {}",
        "floodfills",
    ),
    (
        "<b>Transit Tunnels:</b> 157",
        "<br>",
        "<b>Transit Tunnels:</b> {}",
        "tunnels.participating",
    ),
];

const NOT_AN_INTEGER: [&str; 10] = [
    "30,21",
    "3021.5",
    "-3021",
    "+3021",
    "\u{ff13}\u{ff10}",
    "3 021",
    "",
    "abc",
    "99999999999999999999",
    "3021x",
];

fn check_i2pd_count_set_to(from: &str, end: &str, template: &str, field: &str, bad: &str) {
    let to = template.replace("{}", bad);
    let got = i2pd_with(&format!("{from}{end}"), &format!("{to}{end}"));
    check(
        &format!("{field} = {bad}"),
        &got,
        &expected_i2pd(),
        &[field],
        &[],
    );
}

#[test]
fn r38_i2pd_counts_that_are_not_integers_are_null_one_by_one() {
    for (from, end, template, field) in I2PD_COUNTS {
        for bad in NOT_AN_INTEGER {
            check_i2pd_count_set_to(from, end, template, field, bad);
        }
        let to = template.replace("{}", "0");
        let got = i2pd_with(&format!("{from}{end}"), &format!("{to}{end}"));
        assert!(
            fields(&got)
                .iter()
                .any(|(n, v)| *n == field && v.as_deref() == Some("0"))
        );
    }
}

#[test]
fn r38_i2pd_counts_need_their_own_terminator() {
    // Routers, Floodfills: `&nbsp;`. Transit Tunnels: `<br>`.
    let wrong = [
        ("3021&nbsp;", "3021<br>", "known_routers"),
        ("812&nbsp;", "812<br>", "floodfills"),
        ("14&nbsp;", "14<br>", "tunnels.client"),
        ("157<br>", "157&nbsp;", "tunnels.participating"),
        ("157<br>", "157</div>", "tunnels.participating"),
    ];
    for (from, to, field) in wrong {
        let got = parse_i2pd_main(&swap(I2PD, from, to));
        check(to, &got, &expected_i2pd(), &[field], &[]);
    }
}

#[test]
fn r38_i2pd_never_reads_the_client_tunnels_figure() {
    // R38: the `Client Tunnels` figure of i2pd counts every inbound and outbound tunnel,
    // exploratory ones included; `tunnels.client` is always null, whatever the line holds.
    let from = "<b>Client Tunnels:</b> 14&nbsp;";
    for line in [
        "<b>Client Tunnels:</b> 14&nbsp;",
        "<b>Client Tunnels:</b> 0&nbsp;",
        "<b>Client Tunnels:</b> 2&nbsp;",
        "<b>Client Tunnels:</b> 99999&nbsp;",
        "<b>Client Tunnels:</b> abc&nbsp;",
        "",
    ] {
        let got = i2pd_with(from, line);
        assert_eq!(got.tunnels.client, None, "line {line:?}");
        check(line, &got, &expected_i2pd(), &["tunnels.client"], &[]);
    }
    let two = format!("{from}<b>Client Tunnels:</b> 3&nbsp;");
    let got = i2pd_with(from, &two);
    assert_eq!(got.tunnels.client, None, "a doubled line");
}

// R38: `<b><label>:</b> `; a translated or changed label gives null for its field.
const I2PD_LABEL_CASES: [(&str, &str, &[&str]); 11] = [
    ("<b>Uptime:</b> 1", "<b>Laufzeit:</b> 1", &UPTIME),
    ("<b>Uptime:</b> 1", "<b>uptime:</b> 1", &UPTIME),
    ("<b>Uptime:</b> 1", "<b>Uptime:</b>1", &UPTIME),
    ("<b>Uptime:</b> 1", "<b>Uptime</b> 1", &UPTIME),
    (
        "<b>Network status:</b> OK",
        "<b>Netzwerkstatus:</b> OK",
        &["network_status"],
    ),
    (
        "<b>Received:</b> 1.23",
        "<b>Received:</b>1.23",
        &["bw.in1s"],
    ),
    ("<b>Sent:</b> 987", "<b>Gesendet:</b> 987", &["bw.out1s"]),
    (
        "<b>Routers:</b> 3021",
        "<b>Routers:</b>3021",
        &["known_routers"],
    ),
    (
        "<b>Floodfills:</b> 812",
        "<b>Floodfills :</b> 812",
        &["floodfills"],
    ),
    (
        "<b>Client Tunnels:</b> 14",
        "<b>Client tunnels:</b> 14",
        &["tunnels.client"],
    ),
    (
        "<b>Transit Tunnels:</b> 157",
        "<b>Transit:</b> 157",
        &["tunnels.participating"],
    ),
];

#[test]
fn r38_i2pd_a_label_is_the_exact_text_with_a_space_after_it() {
    for (from, to, nulls) in I2PD_LABEL_CASES {
        let got = i2pd_with(from, to);
        check(to, &got, &expected_i2pd(), nulls, &[]);
    }
}

#[test]
fn r35_i2pd_a_label_that_is_in_the_body_twice_nulls_its_fields() {
    let cases: [(&str, Vec<&str>); 9] = [
        ("<b>Uptime:</b> 5 seconds<br>\n", UPTIME.to_vec()),
        ("<b>Network status:</b> OK<br>\n", vec!["network_status"]),
        (
            "<b>Tunnel creation success rate:</b> 1%<br>\n",
            vec!["build.total"],
        ),
        ("<b>Received:</b> 1 GiB (9 KiB/s)<br>\n", vec!["bw.in1s"]),
        ("<b>Sent:</b> 1 GiB (9 KiB/s)<br>\n", vec!["bw.out1s"]),
        ("<b>Routers:</b> 5&nbsp;\n", vec!["known_routers"]),
        ("<b>Floodfills:</b> 5&nbsp;\n", vec!["floodfills"]),
        ("<b>Client Tunnels:</b> 5&nbsp;\n", vec!["tunnels.client"]),
        (
            "<b>Transit Tunnels:</b> 5<br>\n",
            vec!["tunnels.participating"],
        ),
    ];
    for (extra, nulls) in cases {
        let body = swap(I2PD, "</body>", &format!("{extra}</body>"));
        let got = parse_i2pd_main(&body);
        check(extra, &got, &expected_i2pd(), &nulls, &[]);
    }
}

#[test]
fn r35_i2pd_a_missing_line_nulls_its_fields_only() {
    let cases: [(&str, Vec<&str>); 3] = [
        (
            "<b>Uptime:</b> 1 day, 2 hours, 3 minutes, 4 seconds<br>\n",
            UPTIME.to_vec(),
        ),
        ("<b>Network status:</b> OK<br>\n", vec!["network_status"]),
        (
            "<b>Tunnel creation success rate:</b> 42%<br>\n",
            vec!["build.total"],
        ),
    ];
    for (line, nulls) in cases {
        let got = parse_i2pd_main(&swap(I2PD, line, ""));
        check(line, &got, &expected_i2pd(), &nulls, &[]);
    }
}

#[test]
fn r38_i2pd_reads_the_english_page_only() {
    // R38: "when the body has no `<html lang="en"`, every field is null."
    for tag in [
        "<html lang=\"de\">",
        "<html lang=\"ru\">",
        "<html>",
        "<html lang=\"\">",
        "",
    ] {
        let got = i2pd_with("<html lang=\"en\">", tag);
        assert_eq!(got, RouterStats::default(), "{tag:?}");
    }
    let got = i2pd_with("<html lang=\"en\">", "<html lang=\"en\" dir=\"ltr\">");
    assert_eq!(got, expected_i2pd());
}

#[test]
fn r38_i2pd_a_value_cut_at_the_end_of_the_body_is_null() {
    // R35, R33: a value at the end of the body has no terminator.
    let end = I2PD.find("157<br>").unwrap() + "157".len();
    let got = parse_i2pd_main(&I2PD[..end]);
    check(
        "cut in Transit",
        &got,
        &expected_i2pd(),
        &["tunnels.participating"],
        &[],
    );

    let end = I2PD.find(" seconds<br>").unwrap() + " seconds".len();
    assert_eq!(parse_i2pd_main(&I2PD[..end]), RouterStats::default());

    // A value that already has its terminator is not cut.
    let end = I2PD.find("3021&nbsp;").unwrap() + "3021&nbsp;".len();
    let got = parse_i2pd_main(&I2PD[..end]);
    let gone = ["floodfills", "tunnels.client", "tunnels.participating"];
    check("cut after Routers", &got, &expected_i2pd(), &gone, &[]);
}

// ------------------------------------------------------------ never a wrong number

/// Every field of `got` is null or its value in `base`.
fn never_wrong(what: &str, got: &RouterStats, base: &RouterStats) {
    let names: Vec<&str> = fields(base).iter().map(|(name, _)| *name).collect();
    check(what, got, base, &[], &names);
}

/// R33, R35: every prefix of `body` parses to null fields or the right ones.
fn every_prefix_never_wrong(kind: ConsoleKind, name: &str, body: &str, base: &RouterStats) {
    for end in (0..=body.len()).filter(|end| body.is_char_boundary(*end)) {
        let got = parse_console_stats(kind, &body[..end]);
        never_wrong(&format!("{name}[..{end}]"), &got, base);
    }
}

#[test]
fn r35_every_prefix_of_a_fixture_parses_without_a_wrong_number() {
    // R33, R35: a body cut anywhere gives null fields or the right ones.
    every_prefix_never_wrong(ConsoleKind::Java, "java", JAVA, &expected_java());
    every_prefix_never_wrong(ConsoleKind::I2pd, "i2pd", I2PD, &expected_i2pd());
}

/// The bytes an edit never writes or removes: `.` (46), `M` (77), and the stand-in `x` (120).
const BYTE_DOT: u8 = 46;
const BYTE_UPPER_M: u8 = 77;
const BYTE_LOWER_X: u8 = 120;

/// The indexes of the bytes an edit may touch: everything but a digit and a dot.
fn free_positions(body: &str) -> Vec<usize> {
    body.bytes()
        .enumerate()
        .filter(|(_, b)| !b.is_ascii_digit() && *b != BYTE_DOT)
        .map(|(i, _)| i)
        .collect()
}

/// The byte an edit writes: never a digit, a dot or an `M`.
fn safe_byte(byte: u8) -> u8 {
    if byte.is_ascii_digit() || byte == BYTE_DOT || byte == BYTE_UPPER_M {
        BYTE_LOWER_X
    } else {
        byte
    }
}

fn replace_byte(bytes: &mut [u8], at: usize, byte: u8) {
    bytes[at] = byte;
}

fn remove_byte(bytes: &mut Vec<u8>, at: usize) {
    bytes.remove(at);
}

fn insert_byte(bytes: &mut Vec<u8>, at: usize, byte: u8) {
    bytes.insert(at, byte);
}

fn apply_edit(bytes: &mut Vec<u8>, at: usize, op: u8, byte: u8) {
    let byte = safe_byte(byte);
    match op % 3 {
        0 => replace_byte(bytes, at, byte),
        1 => remove_byte(bytes, at),
        _ => insert_byte(bytes, at, byte),
    }
}

/// The fixture with up to four edits that never touch a digit or a dot, never write one,
/// and never write an `M` (the one letter that turns `KBps` into another valid unit).
/// Positions are indexes into the bytes the edits may touch.
fn edited(body: &str, edits: &[(usize, u8, u8)]) -> String {
    let free = free_positions(body);
    let mut bytes = body.as_bytes().to_vec();
    let mut edits: Vec<(usize, u8, u8)> = edits
        .iter()
        .map(|(i, op, b)| (free[i % free.len()], *op, *b))
        .collect();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.0));
    for (at, op, byte) in edits {
        apply_edit(&mut bytes, at, op, byte);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn property_config() -> Config {
    Config {
        cases: 800,
        failure_persistence: None,
        ..Config::default()
    }
}

/// R35: every non-null field of `got` equals its value in `base`.
fn only_null_or_original(
    what: &str,
    got: &RouterStats,
    base: &RouterStats,
) -> Result<(), TestCaseError> {
    let names: Vec<&str> = fields(base).iter().map(|(n, _)| *n).collect();
    for ((name, got), (_, want)) in fields(got).into_iter().zip(fields(base)) {
        prop_assert!(
            got.is_none() || got == want,
            "{what}: {name} is {got:?}, the original is {want:?} ({names:?})"
        );
    }
    prop_assert!(got.history.is_empty());
    Ok(())
}

#[test]
fn r35_property_random_byte_edits_never_panic_and_never_give_a_wrong_number() {
    // R35: after random non-digit byte edits of a fixture, every non-null field equals
    // the original value; a parser never panics.
    let strategy = proptest::collection::vec((any::<usize>(), any::<u8>(), any::<u8>()), 1..=4);
    let mut runner = TestRunner::new(property_config());
    let result = runner.run(&strategy, |edits| {
        let java = parse_java_summary(&edited(JAVA, &edits));
        let i2pd = parse_i2pd_main(&edited(I2PD, &edits));
        for (what, got, base) in [
            ("java", java, expected_java()),
            ("i2pd", i2pd, expected_i2pd()),
        ] {
            only_null_or_original(what, &got, &base)?;
        }
        Ok(())
    });
    result.unwrap();
}

#[test]
fn r35_property_any_bytes_never_panic_a_parser() {
    let strategy = proptest::collection::vec(any::<u8>(), 0..2_000);
    let mut runner = TestRunner::new(property_config());
    let result = runner.run(&strategy, |bytes| {
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let _ = parse_java_summary(&text);
        let _ = parse_i2pd_main(&text);
        Ok(())
    });
    result.unwrap();
}

// ------------------------------------------------------------ R32, R33: the request

#[test]
fn r32_the_stats_path_is_fixed_per_router() {
    assert_eq!(stats_path(ConsoleKind::Java), JAVA_PATH);
    assert_eq!(stats_path(ConsoleKind::I2pd), "/");
}

#[test]
fn r32_the_stats_path_never_carries_lang_action_or_a_console_nonce() {
    // R32: a Java I2P console saves `?lang=` in the router configuration.
    for kind in [ConsoleKind::Java, ConsoleKind::I2pd] {
        let path = stats_path(kind);
        for word in ["lang", "action", "consoleNonce"] {
            assert!(!path.contains(word), "{kind:?} path {path:?} holds {word}");
        }
    }
}

#[test]
fn r33_the_bounds_are_3_seconds_and_256_kib() {
    assert_eq!(STATS_TIMEOUT, Duration::from_secs(3));
    assert_eq!(STATS_MAX_ANSWER, 256 * 1024);
}

/// The reply of a Java I2P console to the probe: it carries the marker of R3.
const PROBE_REPLY: &str = "HTTP/1.0 200 OK\r\nContent-Type: text/html\r\n\r\n<link rel=\"stylesheet\" href=\"/themes/console/light/console.css?2.13.0\">";

type OnStats = dyn Fn(&mut TcpStream) + Send + Sync;

/// A loopback server that passes the probe of a Java I2P console and hands the stats
/// request to `on_stats`. It records the head of every request.
struct Scripted {
    port: u16,
    heads: Arc<Mutex<Vec<String>>>,
}

fn accept_loop(listener: &TcpListener, log: &Arc<Mutex<Vec<String>>>, on_stats: &Arc<OnStats>) {
    for stream in listener.incoming().flatten() {
        let (log, on_stats) = (Arc::clone(log), Arc::clone(on_stats));
        thread::spawn(move || script(stream, &log, on_stats.as_ref()));
    }
}

impl Scripted {
    fn start(on_stats: impl Fn(&mut TcpStream) + Send + Sync + 'static) -> Scripted {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let heads = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&heads);
        let on_stats: Arc<OnStats> = Arc::new(on_stats);
        thread::spawn(move || accept_loop(&listener, &log, &on_stats));
        Scripted { port, heads }
    }

    fn console(&self) -> crate::net::console::VerifiedConsole {
        probe(ConsoleKind::Java, self.port).expect("the scripted console passes the probe")
    }

    /// The heads of the requests for the stats path.
    fn stats_heads(&self) -> Vec<String> {
        let heads = self.heads.lock().unwrap().clone();
        heads
            .into_iter()
            .filter(|h| h.split(' ').nth(1) == Some(JAVA_PATH))
            .collect()
    }

    fn all_heads(&self) -> Vec<String> {
        self.heads.lock().unwrap().clone()
    }
}

fn script(mut stream: TcpStream, log: &Mutex<Vec<String>>, on_stats: &OnStats) {
    let head = read_request_head(&mut stream);
    let target = head.split(' ').nth(1).unwrap_or("").to_owned();
    log.lock().unwrap().push(head);
    if target == JAVA_PATH {
        on_stats(&mut stream);
    } else {
        let _ = stream.write_all(PROBE_REPLY.as_bytes());
    }
}

fn read_request_head(stream: &mut TcpStream) -> String {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte).unwrap_or(0) == 0 {
            break;
        }
        head.push(byte[0]);
    }
    String::from_utf8_lossy(&head).into_owned()
}

/// Answers `200` with `body`, then closes.
fn answer_ok(body: String) -> impl Fn(&mut TcpStream) + Send + Sync + 'static {
    move |stream| {
        let reply = format!("HTTP/1.0 200 OK\r\nContent-Type: text/html\r\n\r\n{body}");
        let _ = stream.write_all(reply.as_bytes());
    }
}

#[test]
fn r32_fetch_sends_one_get_with_the_four_headers_of_r4_and_nothing_else() {
    // R32: `GET <path> HTTP/1.0` in origin form; Host, User-Agent: eepview,
    // Accept: text/html, Connection: close; no cookie, no body.
    let server = Scripted::start(answer_ok(JAVA.to_owned()));
    let got = fetch_stats(&server.console());
    assert_eq!(got, Some(expected_java()));
    let heads = server.stats_heads();
    assert_eq!(heads.len(), 1, "one request for the statistics: {heads:?}");
    let mut lines = heads[0].split("\r\n");
    assert_eq!(
        lines.next(),
        Some("GET /xhr1.jsp?requestURI=/summaryframe HTTP/1.0")
    );
    let mut headers: Vec<(String, String)> = lines
        .filter(|line| !line.is_empty())
        .map(|line| {
            let (name, value) = line.split_once(':').expect("a header has a colon");
            (name.trim().to_ascii_lowercase(), value.trim().to_owned())
        })
        .collect();
    headers.sort();
    let want = [
        ("accept", "text/html".to_owned()),
        ("connection", "close".to_owned()),
        ("host", format!("127.0.0.1:{}", server.port)),
        ("user-agent", "eepview".to_owned()),
    ];
    let want: Vec<(String, String)> = want.into_iter().map(|(n, v)| (n.to_owned(), v)).collect();
    assert_eq!(headers, want);
}

#[test]
fn r32_the_request_line_has_no_lang_action_or_nonce() {
    let server = Scripted::start(answer_ok(JAVA.to_owned()));
    let _ = fetch_stats(&server.console());
    let heads = server.stats_heads();
    let line = heads[0].lines().next().unwrap_or_default().to_owned();
    for word in ["lang", "action", "consoleNonce"] {
        assert!(!line.contains(word), "{line}");
    }
    assert!(!heads[0].to_ascii_lowercase().contains("cookie"));
}

#[test]
fn r32_fetch_gives_the_parsed_java_stats_with_an_empty_history() {
    let fake = FakeConsole::serving(ConsoleKind::Java, JAVA_PATH, 200, JAVA);
    let console = fake.verified();
    let before = fake.requests().len();
    let got = fetch_stats(&console);
    assert_eq!(got, Some(expected_java()));
    let sent = fake.requests();
    assert_eq!(
        &sent[before..],
        ["GET /xhr1.jsp?requestURI=/summaryframe HTTP/1.0".to_owned()],
        "one GET of the stats path"
    );
}

#[test]
fn r32_fetch_gives_the_parsed_i2pd_stats() {
    let fake = FakeConsole::serving(ConsoleKind::I2pd, "/", 200, I2PD);
    let got = fetch_stats(&fake.verified());
    assert_eq!(got, Some(expected_i2pd()));
}

#[test]
fn r33_fetch_gives_none_for_a_status_other_than_200() {
    // R33: "a status other than 200 counts as the console does not answer".
    for status in [204, 301, 302, 304, 400, 401, 403, 404, 500, 503] {
        let fake = FakeConsole::serving(ConsoleKind::Java, JAVA_PATH, status, JAVA);
        let console = fake.verified();
        let before = fake.requests().len();
        assert_eq!(fetch_stats(&console), None, "status {status}");
        assert_eq!(
            fake.requests().len(),
            before + 1,
            "status {status}: one request, no retry"
        );
    }
}

#[test]
fn r32_fetch_never_follows_a_redirect() {
    let server = Scripted::start(|stream| {
        let _ = stream.write_all(
            format!("HTTP/1.0 302 Found\r\nLocation: {JAVA_PATH}&x=1\r\n\r\n").as_bytes(),
        );
    });
    assert_eq!(fetch_stats(&server.console()), None);
    assert_eq!(
        server.stats_heads().len(),
        1,
        "no second request after the 302"
    );
    assert!(server.all_heads().iter().all(|h| !h.contains("&x=1")));
}

#[test]
fn r33_fetch_gives_none_for_an_answer_that_is_not_http() {
    let server = Scripted::start(|stream| {
        let _ = stream.write_all(b"this is not http\r\n\r\n<table id=\"sb_tunnels\"></table>");
    });
    assert_eq!(fetch_stats(&server.console()), None);
}

#[test]
fn r33_fetch_gives_none_when_the_connection_closes_without_an_answer() {
    let server = Scripted::start(|_stream| {});
    assert_eq!(fetch_stats(&server.console()), None);
}

#[test]
fn r33_fetch_gives_none_after_3_seconds_without_any_answer() {
    // R33: "a timeout before any answer"; the server would answer after 10 s.
    let server = Scripted::start(|stream| {
        thread::sleep(Duration::from_secs(10));
        let reply = format!("HTTP/1.0 200 OK\r\n\r\n{JAVA}");
        let _ = stream.write_all(reply.as_bytes());
    });
    let console = server.console();
    let start = Instant::now();
    let got = fetch_stats(&console);
    let took = start.elapsed();
    assert_eq!(got, None);
    assert!(
        took < STATS_TIMEOUT + Duration::from_secs(2),
        "waited {took:?}"
    );
}

#[test]
fn r33_a_body_cut_by_the_timeout_is_parsed_and_the_cut_value_is_null() {
    // R33, R35: "A body cut by the size cap or by the timeout is still parsed".
    let end = JAVA.find(">398<").unwrap() + ">39".len();
    let prefix = JAVA[..end].to_owned();
    let server = Scripted::start(move |stream| {
        let _ = stream.write_all(format!("HTTP/1.0 200 OK\r\n\r\n{prefix}").as_bytes());
        let _ = stream.flush();
        thread::sleep(Duration::from_secs(10));
    });
    let console = server.console();
    let start = Instant::now();
    let got = fetch_stats(&console).expect("a cut body is still parsed");
    assert!(start.elapsed() < STATS_TIMEOUT + Duration::from_secs(2));
    let tunnels = ["tunnels.client", "tunnels.exploratory"];
    check(
        "timeout cut",
        &got,
        &expected_java(),
        &["tunnels.participating"],
        &tunnels,
    );
}

/// A body whose first `STATS_MAX_ANSWER` bytes end exactly after `JAVA[..cut]`, then `tail`.
fn capped(cut: usize, tail: &str) -> String {
    let head = &JAVA[..cut];
    let cap = usize::try_from(STATS_MAX_ANSWER).unwrap();
    let pad = cap - head.len() - "<!---->".len();
    let body = format!("<!--{}-->{head}{tail}", "x".repeat(pad));
    assert_eq!(body.find(head).unwrap() + head.len(), cap);
    body
}

#[test]
fn r33_fetch_reads_exactly_the_first_256_kib() {
    // R33: "It reads at most 256 KiB". Everything up to the cap parses; a second
    // sb_peers table after the cap would null the peers if it were read.
    let cut = table_range(JAVA, "sb_tunnels").1;
    let tail = format!("{}{}", table("sb_peers", &PEER_ROWS), &JAVA[cut..]);
    let body = capped(cut, &tail);
    assert!(body.len() > usize::try_from(STATS_MAX_ANSWER).unwrap());
    let fake = FakeConsole::serving(ConsoleKind::Java, JAVA_PATH, 200, &body);
    let got = fetch_stats(&fake.verified());
    assert_eq!(got, Some(expected_java()));
}

#[test]
fn r33_a_value_cut_by_the_size_cap_is_null() {
    // R33, R35: the cap lands inside `398`; the rest of the body is never read.
    let cut = JAVA.find(">398<").unwrap() + ">39".len();
    let body = capped(cut, &JAVA[cut..]);
    let fake = FakeConsole::serving(ConsoleKind::Java, JAVA_PATH, 200, &body);
    let got = fetch_stats(&fake.verified()).expect("a body cut by the cap is still parsed");
    let tunnels = ["tunnels.client", "tunnels.exploratory"];
    check(
        "cap cut",
        &got,
        &expected_java(),
        &["tunnels.participating"],
        &tunnels,
    );
}

// ------------------------------------------------------------ R46: one deadline

/// The longest a call may last: the 3 s of R33 plus the time to close the socket.
const R38_LIMIT: Duration = Duration::from_millis(3_800);

/// A gap shorter than the 3 s of one read, so a deadline that restarts at every read
/// never fires while the peer keeps sending.
const DRIP_GAP: Duration = Duration::from_millis(1_200);

/// Writes `bytes` one at a time, `DRIP_GAP` apart, then holds the socket open for `hold`.
fn drip(stream: &mut TcpStream, bytes: &[u8], hold: Duration) {
    for byte in bytes {
        if stream
            .write_all(&[*byte])
            .and_then(|()| stream.flush())
            .is_err()
        {
            return;
        }
        thread::sleep(DRIP_GAP);
    }
    thread::sleep(hold);
}

#[test]
fn r46_a_peer_that_sends_slowly_past_3_seconds_ends_the_call_at_3_seconds() {
    // R46: the head and the start of the body arrive at once; then one byte every 1.2 s.
    // Each read is inside 3 s, the whole request is not. The call returns at the deadline.
    let end = JAVA.find(">398<").unwrap() + ">3".len();
    let prefix = JAVA[..end].to_owned();
    let server = Scripted::start(move |stream| {
        let _ = stream.write_all(format!("HTTP/1.0 200 OK\r\n\r\n{prefix}").as_bytes());
        drip(stream, b"98<", Duration::from_secs(8));
    });
    let console = server.console();
    let start = Instant::now();
    let got = fetch_stats(&console);
    let took = start.elapsed();
    assert!(took < R38_LIMIT, "the call lasted {took:?}");
    assert!(got.is_some(), "a body cut by the deadline is still parsed");
}

#[test]
fn r46_a_value_cut_by_the_deadline_is_null() {
    // R46, R35: `398` arrives at 2.4 s but its terminator at 3.6 s, after the deadline.
    let end = JAVA.find(">398<").unwrap() + ">3".len();
    let prefix = JAVA[..end].to_owned();
    let server = Scripted::start(move |stream| {
        let _ = stream.write_all(format!("HTTP/1.0 200 OK\r\n\r\n{prefix}").as_bytes());
        drip(stream, b"98<", Duration::from_secs(8));
    });
    let console = server.console();
    let start = Instant::now();
    let got = fetch_stats(&console).expect("a body cut by the deadline is still parsed");
    assert!(
        start.elapsed() < R38_LIMIT,
        "the call lasted {:?}",
        start.elapsed()
    );
    let tunnels = ["tunnels.client", "tunnels.exploratory"];
    check(
        "deadline cut",
        &got,
        &expected_java(),
        &["tunnels.participating"],
        &tunnels,
    );
}

#[test]
fn r46_figures_that_arrived_before_the_deadline_are_kept() {
    // R46: "gets figures from what arrived within 3 s".
    let end = JAVA.find(">398<").unwrap() + ">3".len();
    let prefix = JAVA[..end].to_owned();
    let server = Scripted::start(move |stream| {
        let _ = stream.write_all(format!("HTTP/1.0 200 OK\r\n\r\n{prefix}").as_bytes());
        drip(stream, b"98<", Duration::from_secs(8));
    });
    let got = fetch_stats(&server.console()).expect("a cut body is still parsed");
    assert_eq!(got.uptime_ms, Some(28_800_000));
    assert_eq!(got.known_routers, Some(4905));
    assert_eq!(got.floodfills, Some(1570));
    assert_eq!(got.bandwidth_bytes_per_second.in1s, Some(53_910));
}

#[test]
fn r46_no_complete_head_within_3_seconds_means_the_console_does_not_answer() {
    // R46, R33: the status line is at once, the head never ends; a header byte comes every
    // 1.2 s. At 3 s there is no complete head: no answer, and the call ends at 3 s.
    let server = Scripted::start(|stream| {
        let _ = stream.write_all(b"HTTP/1.0 200 OK\r\n");
        drip(stream, b"X-Slow: aaaaaaaaaa", Duration::from_secs(8));
    });
    let console = server.console();
    let start = Instant::now();
    let got = fetch_stats(&console);
    let took = start.elapsed();
    assert_eq!(got, None);
    assert!(took < R38_LIMIT, "the call lasted {took:?}");
}

#[test]
fn r46_a_head_that_completes_after_3_seconds_means_the_console_does_not_answer() {
    // R46: the head ends at 3.6 s, with a full body behind it: too late, no answer.
    let body = JAVA.to_owned();
    let server = Scripted::start(move |stream| {
        let _ = stream.write_all(b"HTTP/1.0 200 OK\r\n");
        drip(stream, b"X:a", Duration::ZERO);
        let _ = stream.write_all(format!("\r\n\r\n{body}").as_bytes());
    });
    let console = server.console();
    let start = Instant::now();
    let got = fetch_stats(&console);
    let took = start.elapsed();
    assert_eq!(got, None);
    assert!(took < R38_LIMIT, "the call lasted {took:?}");
}
