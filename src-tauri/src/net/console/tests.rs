// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests for the router console (`docs/wiki/router-console.md`): detection
//! (R1 to R5), the page map (R7) and the navigation route (R9, R10). They use only the
//! public interface and the `FakeConsole` helper.

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use serde_json::json;
use tauri::test::{MockRuntime, mock_builder, mock_context, noop_assets};
use tauri::{App, AppHandle, Listener, Manager, Url};

use crate::core::Core;
use crate::net::console::{
    ConsoleInfo, ConsoleKind, ConsoleNav, ConsolePage, I2PD_DEFAULT_PORT, JAVA_DEFAULT_PORT,
    RECHECK_MISSES, after_recheck, candidates, console_version, detect, detect_here,
    i2pd_config_files, i2pd_console_port, i2pd_port_in, java_config_dirs, java_console_port,
    java_ports_in, judge, page_path, probe, probe_path,
};
use crate::net::testing::FakeConsole;
use crate::shell::console::{ConsoleWebview, current, detect_now, set_console, stop};
use crate::shell::state::Shared;

const JAVA_BODY: &str = r#"<link rel="stylesheet" href="/themes/console/light/console.css">"#;
const I2PD_BODY: &str = r#"<a href="/?page=i2p_tunnels">Tunnels</a>"#;

// ---------------------------------------------------------------- helpers

fn url(text: &str) -> Url {
    Url::parse(text).unwrap()
}

fn judged(kind: ConsoleKind, code: u16, body: &str) -> bool {
    judge(kind, &Ok((code, body.to_owned())))
}

/// A plain HTTP server that answers every request the same way and records the heads.
struct Plain {
    port: u16,
    heads: Arc<Mutex<Vec<String>>>,
}

impl Plain {
    fn start(status: &str, headers: &str, body: &str) -> Plain {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let heads = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&heads);
        let reply = format!(
            "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        thread::spawn(move || serve_plain(&listener, &reply, &log));
        Plain { port, heads }
    }

    fn heads(&self) -> Vec<String> {
        self.heads.lock().unwrap().clone()
    }
}

fn serve_plain(listener: &TcpListener, reply: &str, log: &Mutex<Vec<String>>) {
    for stream in listener.incoming().flatten() {
        reply_to(stream, reply, log);
    }
}

fn reply_to(mut stream: TcpStream, reply: &str, log: &Mutex<Vec<String>>) {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte).unwrap_or(0) == 0 {
            break;
        }
        head.push(byte[0]);
    }
    log.lock()
        .unwrap()
        .push(String::from_utf8_lossy(&head).into_owned());
    let _ = stream.write_all(reply.as_bytes());
}

/// A loopback port where nothing listens.
fn closed_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// A home folder in the temp dir, removed on drop.
struct Home(PathBuf);

impl Home {
    fn new() -> Home {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("eepview-console-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        Home(dir)
    }

    fn env(&self) -> impl Fn(&str) -> Option<String> + use<> {
        let text = self.0.to_string_lossy().into_owned();
        move |key| match key {
            "HOME" | "LOCALAPPDATA" | "APPDATA" => Some(text.clone()),
            _ => None,
        }
    }

    /// The Java I2P configuration folder of this OS (R2).
    fn java_dir(&self) -> PathBuf {
        if cfg!(windows) {
            self.0.join("I2P")
        } else if cfg!(target_os = "macos") {
            self.0
                .join("Library")
                .join("Application Support")
                .join("i2p")
        } else {
            self.0.join(".i2p")
        }
    }

    /// The `i2pd.conf` of this OS (R2).
    fn i2pd_file(&self) -> PathBuf {
        if cfg!(windows) {
            self.0.join("i2pd").join("i2pd.conf")
        } else if cfg!(target_os = "macos") {
            self.0
                .join("Library")
                .join("Application Support")
                .join("i2pd")
                .join("i2pd.conf")
        } else {
            self.0.join(".i2pd").join("i2pd.conf")
        }
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn java_clients(port: u16) -> String {
    format!(
        "clientApp.0.main=net.i2p.router.web.RouterConsoleRunner\n\
         clientApp.0.args={port} ::1,127.0.0.1 ./webapps/\n\
         clientApp.0.name=I2P Router Console\n\
         clientApp.0.startOnLoad=true\n"
    )
}

fn i2pd_conf(port: u16) -> String {
    format!("[http]\nenabled = true\naddress = 127.0.0.1\nport = {port}\n")
}

fn no_env() -> impl Fn(&str) -> Option<String> {
    |_| None
}

// ---------------------------------------------------------------- R1

#[test]
fn r1_without_config_the_candidates_are_the_two_defaults_in_order() {
    // R1: java default 7657, then i2pd default 7070.
    assert_eq!(JAVA_DEFAULT_PORT, 7657);
    assert_eq!(I2PD_DEFAULT_PORT, 7070);
    let got = candidates(&no_env());
    let defaults = [(ConsoleKind::Java, 7657), (ConsoleKind::I2pd, 7070)];
    assert!(got.ends_with(&defaults), "{got:?}");
    if !cfg!(target_os = "linux") {
        assert_eq!(got, defaults);
    }
}

#[test]
fn r1_configured_ports_come_before_the_defaults_java_first() {
    // R1: java config ports, then i2pd config port, then the two defaults.
    let home = Home::new();
    write(&home.java_dir().join("clients.config"), &java_clients(7658));
    write(&home.i2pd_file(), &i2pd_conf(7071));
    let got = candidates(&home.env());
    let first = [
        (ConsoleKind::Java, 7658),
        (ConsoleKind::I2pd, 7071),
        (ConsoleKind::Java, 7657),
        (ConsoleKind::I2pd, 7070),
    ];
    assert_eq!(got, first);
}

#[test]
fn r1_a_configured_default_port_is_not_listed_twice() {
    // R1: no duplicates.
    let home = Home::new();
    write(&home.java_dir().join("clients.config"), &java_clients(7657));
    write(&home.i2pd_file(), &i2pd_conf(7070));
    let got = candidates(&home.env());
    assert_eq!(got, [(ConsoleKind::Java, 7657), (ConsoleKind::I2pd, 7070)]);
}

#[test]
fn r1_several_java_ports_keep_the_file_order() {
    // R1 and R2: clients.config.d by file name, then clients.config.
    let home = Home::new();
    let dir = home.java_dir();
    write(
        &dir.join("clients.config.d").join("b.config"),
        &java_clients(7660),
    );
    write(
        &dir.join("clients.config.d").join("a.config"),
        &java_clients(7659),
    );
    write(&dir.join("clients.config"), &java_clients(7658));
    let got = candidates(&home.env());
    let java: Vec<u16> = got
        .iter()
        .filter(|(k, _)| *k == ConsoleKind::Java)
        .map(|(_, p)| *p)
        .collect();
    assert_eq!(java, [7659, 7660, 7658, 7657]);
}

// ---------------------------------------------------------------- R2: file text

#[test]
fn r2_java_console_port_is_the_first_number_of_the_console_args() {
    // R2: clientApp.<n>.args = "7657 ::1,127.0.0.1 ./webapps/".
    assert_eq!(java_console_port(&java_clients(7657)), Some(7657));
    assert_eq!(java_console_port(&java_clients(17657)), Some(17657));
}

#[test]
fn r2_java_console_is_found_among_other_clients() {
    // R2: the client whose main is RouterConsoleRunner, whatever its number.
    let text = "clientApp.0.main=net.i2p.sam.SAMBridge\n\
                clientApp.0.args=7656 127.0.0.1\n\
                clientApp.1.main=net.i2p.router.web.RouterConsoleRunner\n\
                clientApp.1.args=7777 ::1,127.0.0.1 ./webapps/\n";
    assert_eq!(java_console_port(text), Some(7777));
}

#[test]
fn r2_java_tls_port_after_dash_s_is_skipped() {
    // R2: a number after -s is a TLS port.
    let head = "clientApp.0.main=net.i2p.router.web.RouterConsoleRunner\n";
    let plain_first = format!("{head}clientApp.0.args=7657 ::1 -s 7667 ::1 ./webapps/\n");
    assert_eq!(java_console_port(&plain_first), Some(7657));
    let tls_first = format!("{head}clientApp.0.args=-s 7667 ::1 7657 ./webapps/\n");
    assert_eq!(java_console_port(&tls_first), Some(7657));
    let tls_only = format!("{head}clientApp.0.args=-s 7667 ::1 ./webapps/\n");
    assert_eq!(java_console_port(&tls_only), None);
}

#[test]
fn r2_java_client_with_start_on_load_false_gives_no_port() {
    // R2: startOnLoad=false gives no port.
    let off = java_clients(7657).replace("startOnLoad=true", "startOnLoad=false");
    assert_eq!(java_console_port(&off), None);
}

#[test]
fn r2_java_text_without_a_console_client_gives_no_port() {
    // R2: no RouterConsoleRunner client, no port.
    assert_eq!(java_console_port(""), None);
    let sam = "clientApp.0.main=net.i2p.sam.SAMBridge\nclientApp.0.args=7656 127.0.0.1\n";
    assert_eq!(java_console_port(sam), None);
}

#[test]
fn r2_i2pd_port_is_the_port_key_of_the_http_section() {
    // R2: the port key in [http].
    assert_eq!(i2pd_console_port(&i2pd_conf(7071)), Some(7071));
    assert_eq!(i2pd_console_port("[http]\nport=7072\n"), Some(7072));
}

#[test]
fn r2_i2pd_keys_outside_the_http_section_are_ignored() {
    // R2: only [http] counts; other sections and the top level do not.
    assert_eq!(
        i2pd_console_port("port = 4447\n[httpproxy]\nport = 4444\n"),
        None
    );
    let mixed =
        "port = 4447\n[httpproxy]\nport = 4444\n[http]\nport = 7071\n[socksproxy]\nport = 4447\n";
    assert_eq!(i2pd_console_port(mixed), Some(7071));
    assert_eq!(
        i2pd_console_port("[http]\nport = 7071\n[httpproxy]\nport = 4444\n"),
        Some(7071)
    );
}

#[test]
fn r2_i2pd_enabled_false_gives_no_port() {
    // R2: enabled = false in [http].
    assert_eq!(
        i2pd_console_port("[http]\nenabled = false\nport = 7071\n"),
        None
    );
    assert_eq!(
        i2pd_console_port("[http]\nport = 7071\nenabled = false\n"),
        None
    );
}

#[test]
fn r2_i2pd_hash_starts_a_comment() {
    // R2: # starts a comment.
    assert_eq!(i2pd_console_port("[http]\n# port = 9999\n"), None);
    assert_eq!(
        i2pd_console_port("[http]\nport = 7071 # the console\n"),
        Some(7071)
    );
    assert_eq!(i2pd_console_port("# [http]\nport = 7071\n"), None);
}

#[test]
fn r2_i2pd_text_without_a_port_gives_none() {
    // R2: no port key, no port.
    assert_eq!(i2pd_console_port(""), None);
    assert_eq!(i2pd_console_port("[http]\nenabled = true\n"), None);
}

// ---------------------------------------------------------------- R2: files and folders

#[test]
fn r2_java_ports_in_reads_the_d_folder_by_name_then_clients_config() {
    // R2: every file in clients.config.d/ by file name, then clients.config.
    let home = Home::new();
    let dir = home.java_dir();
    write(
        &dir.join("clients.config.d").join("20-b.config"),
        &java_clients(7660),
    );
    write(
        &dir.join("clients.config.d").join("10-a.config"),
        &java_clients(7659),
    );
    write(&dir.join("clients.config"), &java_clients(7658));
    assert_eq!(java_ports_in(&dir), [7659, 7660, 7658]);
}

#[test]
fn r2_java_ports_in_skips_clients_that_do_not_start() {
    // R2: startOnLoad=false gives no port, in any file.
    let home = Home::new();
    let dir = home.java_dir();
    let off = java_clients(7659).replace("startOnLoad=true", "startOnLoad=false");
    write(&dir.join("clients.config.d").join("a.config"), &off);
    write(&dir.join("clients.config"), &java_clients(7658));
    assert_eq!(java_ports_in(&dir), [7658]);
}

#[test]
fn r2_a_missing_folder_or_file_gives_no_port() {
    // R2: a missing or unreadable file gives no port.
    let home = Home::new();
    assert!(java_ports_in(&home.java_dir()).is_empty());
    assert_eq!(i2pd_port_in(&home.i2pd_file()), None);
}

#[test]
fn r2_i2pd_port_in_reads_the_file() {
    // R2: the port key of [http] in the file.
    let home = Home::new();
    write(&home.i2pd_file(), &i2pd_conf(7071));
    assert_eq!(i2pd_port_in(&home.i2pd_file()), Some(7071));
}

#[test]
fn r2_java_config_dirs_are_the_folders_of_this_os() {
    // R2: macOS Library/Application Support/i2p; Linux ~/.i2p and /var/lib/i2p/i2p-config;
    // Windows %LOCALAPPDATA%\I2P and %APPDATA%\I2P.
    let env = |key: &str| (key == "HOME").then(|| "/home/u".to_owned());
    let got = java_config_dirs(&env);
    if cfg!(target_os = "macos") {
        let want = Path::new("/home/u")
            .join("Library")
            .join("Application Support")
            .join("i2p");
        assert_eq!(got, [want]);
    } else if cfg!(target_os = "linux") {
        let want = [
            Path::new("/home/u").join(".i2p"),
            PathBuf::from("/var/lib/i2p/i2p-config"),
        ];
        assert_eq!(got, want);
    }
}

#[test]
fn r2_i2pd_config_files_are_the_files_of_this_os() {
    // R2: macOS Library/Application Support/i2pd/i2pd.conf; Linux ~/.i2pd/i2pd.conf and
    // /etc/i2pd/i2pd.conf; Windows %APPDATA%\i2pd\i2pd.conf.
    let env = |key: &str| (key == "HOME").then(|| "/home/u".to_owned());
    let got = i2pd_config_files(&env);
    if cfg!(target_os = "macos") {
        let base = Path::new("/home/u")
            .join("Library")
            .join("Application Support");
        assert_eq!(got, [base.join("i2pd").join("i2pd.conf")]);
    } else if cfg!(target_os = "linux") {
        let want = [
            Path::new("/home/u").join(".i2pd").join("i2pd.conf"),
            PathBuf::from("/etc/i2pd/i2pd.conf"),
        ];
        assert_eq!(got, want);
    }
}

#[test]
fn r2_a_path_whose_variable_is_not_set_is_left_out() {
    // R2: no HOME, no home-based path.
    let java = java_config_dirs(&no_env());
    let i2pd = i2pd_config_files(&no_env());
    for path in java.iter().chain(i2pd.iter()) {
        let text = path.to_string_lossy();
        assert!(
            !text.contains("Library") && !text.contains(".i2p"),
            "{text}"
        );
    }
    if cfg!(target_os = "macos") || cfg!(windows) {
        assert!(java.is_empty() && i2pd.is_empty(), "{java:?} {i2pd:?}");
    }
}

// ---------------------------------------------------------------- R3: judge

#[test]
fn r3_probe_paths_are_home_and_root() {
    // R3: Java I2P probes /home, i2pd probes /.
    assert_eq!(probe_path(ConsoleKind::Java), "/home");
    assert_eq!(probe_path(ConsoleKind::I2pd), "/");
}

#[test]
fn r3_judge_accepts_the_marker_of_its_own_console() {
    // R3: 200 and the marker.
    assert!(judged(ConsoleKind::Java, 200, JAVA_BODY));
    assert!(judged(ConsoleKind::I2pd, 200, I2PD_BODY));
}

#[test]
fn r3_judge_needs_both_java_markers() {
    // R3: /themes/console/ and console.css, both.
    let themes_only = r#"<link href="/themes/console/light/style.css">"#;
    let css_only = r#"<link href="/static/console.css">"#;
    assert!(!judged(ConsoleKind::Java, 200, themes_only));
    assert!(!judged(ConsoleKind::Java, 200, css_only));
    assert!(!judged(ConsoleKind::Java, 200, ""));
}

#[test]
fn r3_judge_rejects_a_body_without_the_marker() {
    // R3: a body without the marker does not count.
    let page = "<html><body>Welcome</body></html>";
    assert!(!judged(ConsoleKind::Java, 200, page));
    assert!(!judged(ConsoleKind::I2pd, 200, page));
    assert!(!judged(ConsoleKind::I2pd, 200, "?page=tunnels"));
}

#[test]
fn r3_judge_rejects_any_status_but_200() {
    // R3: another status does not count, even with the marker.
    for code in [201, 204, 301, 302, 401, 403, 404, 500, 503] {
        assert!(!judged(ConsoleKind::Java, code, JAVA_BODY), "{code}");
        assert!(!judged(ConsoleKind::I2pd, code, I2PD_BODY), "{code}");
    }
}

#[test]
fn r3_judge_rejects_a_failed_probe() {
    // R3: a closed port gives an error answer.
    assert!(!judge(ConsoleKind::Java, &Err("refused".to_owned())));
    assert!(!judge(ConsoleKind::I2pd, &Err("refused".to_owned())));
}

#[test]
fn r3_judge_rejects_the_console_of_the_other_router() {
    // R3: the console of the other router type does not count.
    assert!(!judged(ConsoleKind::Java, 200, I2PD_BODY));
    assert!(!judged(ConsoleKind::I2pd, 200, JAVA_BODY));
}

#[test]
fn r3_markers_do_not_depend_on_the_console_language() {
    // R3: the markers are in the markup, not in the translated text.
    let java = format!(
        "<p>Routerkonsole \u{2014} \u{4e2d}\u{6587} \u{0440}\u{0443}\u{0441}</p>{JAVA_BODY}"
    );
    let i2pd = format!("<h1>\u{30eb}\u{30fc}\u{30bf}\u{30fc}</h1>{I2PD_BODY}");
    assert!(judged(ConsoleKind::Java, 200, &java));
    assert!(judged(ConsoleKind::I2pd, 200, &i2pd));
}

// ---------------------------------------------------------------- R3, R4: probe

#[test]
fn r3_probe_accepts_a_fake_console_of_the_same_kind() {
    // R3: the answer looks like that router's console.
    for kind in [ConsoleKind::Java, ConsoleKind::I2pd] {
        let fake = FakeConsole::start(kind);
        let found = probe(kind, fake.port()).expect("console accepted");
        assert_eq!(found.kind(), kind);
        assert_eq!(found.port(), fake.port());
    }
}

#[test]
fn r3_probe_rejects_a_fake_console_of_the_other_kind() {
    // R3: the console of the other router type does not count.
    let i2pd = FakeConsole::start(ConsoleKind::I2pd);
    assert!(probe(ConsoleKind::Java, i2pd.port()).is_none());
    let java = FakeConsole::start(ConsoleKind::Java);
    assert!(probe(ConsoleKind::I2pd, java.port()).is_none());
}

#[test]
fn r3_probe_rejects_a_plain_http_server() {
    // R3: a port counts only when the answer looks like that console.
    let plain = Plain::start("200 OK", "Content-Type: text/html\r\n", "<h1>hello</h1>");
    assert!(probe(ConsoleKind::Java, plain.port).is_none());
    assert!(probe(ConsoleKind::I2pd, plain.port).is_none());
}

#[test]
fn r3_probe_rejects_a_closed_port() {
    // R3: a closed port does not count.
    let port = closed_port();
    assert!(probe(ConsoleKind::Java, port).is_none());
    assert!(probe(ConsoleKind::I2pd, port).is_none());
}

#[test]
fn r3_probe_rejects_another_status_with_the_marker() {
    // R3: another status does not count.
    let plain = Plain::start("404 Not Found", "", JAVA_BODY);
    assert!(probe(ConsoleKind::Java, plain.port).is_none());
    let plain = Plain::start("500 Internal Server Error", "", I2PD_BODY);
    assert!(probe(ConsoleKind::I2pd, plain.port).is_none());
}

#[test]
fn r3_probe_asks_the_probe_path_of_the_kind() {
    // R3: Java GET /home, i2pd GET /.
    for kind in [ConsoleKind::Java, ConsoleKind::I2pd] {
        let fake = FakeConsole::start(kind);
        assert!(probe(kind, fake.port()).is_some());
        let want = format!("GET {} ", probe_path(kind));
        let lines = fake.requests();
        assert!(!lines.is_empty());
        assert!(lines.iter().all(|l| l.starts_with(&want)), "{lines:?}");
    }
}

#[test]
fn r4_probe_sends_only_get_with_a_loopback_host_header() {
    // R4: only GET, Host: 127.0.0.1:<port>.
    let plain = Plain::start("200 OK", "", JAVA_BODY);
    assert!(probe(ConsoleKind::Java, plain.port).is_some());
    let heads = plain.heads();
    assert_eq!(heads.len(), 1, "one request: {heads:?}");
    assert!(heads[0].starts_with("GET /home "), "{}", heads[0]);
    let host = format!("host: 127.0.0.1:{}", plain.port);
    assert!(
        heads[0].to_ascii_lowercase().contains(&host),
        "{}",
        heads[0]
    );
}

#[test]
fn r4_probe_never_follows_a_redirect() {
    // R4: it never follows a redirect, even to a real console.
    let target = FakeConsole::start(ConsoleKind::Java);
    let location = format!("Location: http://127.0.0.1:{}/home\r\n", target.port());
    let plain = Plain::start("302 Found", &location, "");
    assert!(probe(ConsoleKind::Java, plain.port).is_none());
    assert!(target.requests().is_empty(), "{:?}", target.requests());
}

#[test]
fn r4_probe_with_a_redirect_that_has_the_marker_still_does_not_count() {
    // R4 and R3: 301 with the marker body is not a 200.
    let plain = Plain::start("301 Moved Permanently", "Location: /home\r\n", JAVA_BODY);
    assert!(probe(ConsoleKind::Java, plain.port).is_none());
    assert_eq!(plain.heads().len(), 1);
}

// ---------------------------------------------------------------- R5: detect

#[test]
fn r5_detect_gives_the_first_candidate_that_passes() {
    // R5: the first passing candidate is the console.
    let java = FakeConsole::start(ConsoleKind::Java);
    let i2pd = FakeConsole::start(ConsoleKind::I2pd);
    let order = [
        (ConsoleKind::I2pd, i2pd.port()),
        (ConsoleKind::Java, java.port()),
    ];
    let found = detect(&order).expect("a console");
    assert_eq!(found, i2pd.verified());
    let reverse = [
        (ConsoleKind::Java, java.port()),
        (ConsoleKind::I2pd, i2pd.port()),
    ];
    assert_eq!(detect(&reverse).expect("a console"), java.verified());
}

#[test]
fn r5_detect_skips_candidates_that_fail() {
    // R5: closed ports, plain servers and wrong-kind consoles are passed over.
    let plain = Plain::start("200 OK", "", "hello");
    let other = FakeConsole::start(ConsoleKind::I2pd);
    let good = FakeConsole::start(ConsoleKind::Java);
    let order = [
        (ConsoleKind::Java, closed_port()),
        (ConsoleKind::Java, plain.port),
        (ConsoleKind::Java, other.port()),
        (ConsoleKind::Java, good.port()),
    ];
    assert_eq!(detect(&order), Some(good.verified()));
}

#[test]
fn r5_detect_gives_none_when_no_candidate_passes() {
    // R5: if none passes, there is no console.
    let plain = Plain::start("200 OK", "", "hello");
    let order = [
        (ConsoleKind::Java, closed_port()),
        (ConsoleKind::I2pd, plain.port),
    ];
    assert_eq!(detect(&order), None);
    assert_eq!(detect(&[]), None);
}

#[test]
fn r5_a_verified_console_reports_its_kind_port_and_origin() {
    // R5 and R9: the origin is http://127.0.0.1:<port>.
    let fake = FakeConsole::start(ConsoleKind::I2pd);
    let console = fake.verified();
    assert_eq!(console.kind(), ConsoleKind::I2pd);
    assert_eq!(console.port(), fake.port());
    assert_eq!(
        console.origin(),
        format!("http://127.0.0.1:{}", fake.port())
    );
}

// ---------------------------------------------------------------- R7: pages

#[test]
fn r7_pages_are_listed_in_the_table_order_with_their_keys() {
    // R7: home, tunnels, addressbook, config, logs.
    use ConsolePage::{AddressBook, Config, Home, Logs, Tunnels};
    assert_eq!(ConsolePage::ALL, [Home, Tunnels, AddressBook, Config, Logs]);
    let keys: Vec<&str> = ConsolePage::ALL.iter().map(|p| p.key()).collect();
    assert_eq!(keys, ["home", "tunnels", "addressbook", "config", "logs"]);
}

#[test]
fn r7_page_keys_parse_back_and_unknown_keys_do_not() {
    // R7 and R12: a known key parses, an unknown one is None.
    for page in ConsolePage::ALL {
        assert_eq!(ConsolePage::parse(page.key()), Some(page));
    }
    for bad in ["", "Home", "address-book", "/home", "settings", "home "] {
        assert_eq!(ConsolePage::parse(bad), None, "{bad:?}");
    }
}

#[test]
fn r7_java_page_paths() {
    // R7: Java I2P column.
    use ConsolePage::{AddressBook, Config, Home, Logs, Tunnels};
    let want = [
        (Home, "/home"),
        (Tunnels, "/tunnels"),
        (AddressBook, "/dns"),
        (Config, "/config"),
        (Logs, "/logs"),
    ];
    for (page, path) in want {
        assert_eq!(page_path(ConsoleKind::Java, page), Some(path), "{page:?}");
    }
}

#[test]
fn r7_i2pd_page_paths_and_missing_pages() {
    // R7: i2pd column; no address book and no logs.
    use ConsolePage::{AddressBook, Config, Home, Logs, Tunnels};
    assert_eq!(page_path(ConsoleKind::I2pd, Home), Some("/"));
    assert_eq!(
        page_path(ConsoleKind::I2pd, Tunnels),
        Some("/?page=tunnels")
    );
    assert_eq!(
        page_path(ConsoleKind::I2pd, Config),
        Some("/?page=commands")
    );
    assert_eq!(page_path(ConsoleKind::I2pd, AddressBook), None);
    assert_eq!(page_path(ConsoleKind::I2pd, Logs), None);
}

#[test]
fn r7_a_console_lists_only_the_pages_its_router_has() {
    // R7: a dash means no link.
    use ConsolePage::{AddressBook, Config, Home, Logs, Tunnels};
    let java = FakeConsole::start(ConsoleKind::Java).verified();
    assert_eq!(java.pages(), [Home, Tunnels, AddressBook, Config, Logs]);
    let i2pd = FakeConsole::start(ConsoleKind::I2pd).verified();
    assert_eq!(i2pd.pages(), [Home, Tunnels, Config]);
}

#[test]
fn r7_page_urls_are_the_origin_plus_the_path() {
    // R7 and R9: the URL of a page is on the detected origin.
    let fake = FakeConsole::start(ConsoleKind::Java);
    let java = fake.verified();
    let base = format!("http://127.0.0.1:{}", fake.port());
    assert_eq!(
        java.url(ConsolePage::Home).unwrap().as_str(),
        format!("{base}/home")
    );
    assert_eq!(
        java.url(ConsolePage::AddressBook).unwrap().as_str(),
        format!("{base}/dns")
    );
    let fake = FakeConsole::start(ConsoleKind::I2pd);
    let i2pd = fake.verified();
    let base = format!("http://127.0.0.1:{}", fake.port());
    assert_eq!(
        i2pd.url(ConsolePage::Home).unwrap().as_str(),
        format!("{base}/")
    );
    assert_eq!(
        i2pd.url(ConsolePage::Config).unwrap().as_str(),
        format!("{base}/?page=commands")
    );
    assert!(i2pd.url(ConsolePage::Logs).is_none());
    assert!(i2pd.url(ConsolePage::AddressBook).is_none());
}

#[test]
fn r7_info_lists_found_kind_origin_and_pages() {
    // R7 and R13: the wire shape is camelCase with lower-case names.
    let fake = FakeConsole::start(ConsoleKind::I2pd);
    let info = fake.verified().info();
    let want = json!({
        "found": true,
        "kind": "i2pd",
        "origin": format!("http://127.0.0.1:{}", fake.port()),
        "pages": ["home", "tunnels", "config"],
        "version": "2.13.0",
    });
    assert_eq!(serde_json::to_value(&info).unwrap(), want);
}

#[test]
fn r7_no_console_info_is_found_false_with_nulls_and_no_pages() {
    // R15 and R13: none() is found false, kind and origin null, no pages.
    let none = ConsoleInfo::none();
    assert!(!none.found);
    assert!(none.kind.is_none() && none.origin.is_none() && none.pages.is_empty());
    assert!(none.version.is_none());
    let want = json!({"found": false, "kind": null, "origin": null, "pages": [], "version": null});
    assert_eq!(serde_json::to_value(&none).unwrap(), want);
}

// ---------------------------------------------------------------- R18: version

#[test]
fn r18_java_version_is_the_query_of_the_console_stylesheet_link() {
    // R18: Java I2P, the version in `console.css?<version>`.
    let page = r#"<link rel="stylesheet" href="/themes/console/light/console.css?2.13.0">"#;
    assert_eq!(
        console_version(ConsoleKind::Java, page),
        Some("2.13.0".to_owned())
    );
    let other =
        r#"<link rel="stylesheet" href="/themes/console/dark/console.css?2.9.0" type="text/css">"#;
    assert_eq!(
        console_version(ConsoleKind::Java, other),
        Some("2.9.0".to_owned())
    );
}

#[test]
fn r18_i2pd_version_follows_the_translated_label() {
    // R18: i2pd, the first `:</b> <version><br>`; the label before it is translated.
    let english = "<b>Version:</b> 2.59.0<br>";
    assert_eq!(
        console_version(ConsoleKind::I2pd, english),
        Some("2.59.0".to_owned())
    );
    let russian = "<b>\u{412}\u{435}\u{440}\u{441}\u{438}\u{44f}:</b> 2.58.0<br>";
    assert_eq!(
        console_version(ConsoleKind::I2pd, russian),
        Some("2.58.0".to_owned())
    );
}

#[test]
fn r18_i2pd_takes_the_first_value_of_digits_and_dots() {
    // R18: the first match whose value is digits and dots.
    let page = "<b>Uptime:</b> 3 hours<br><b>Version:</b> 2.59.0<br><b>Other:</b> 9.9<br>";
    assert_eq!(
        console_version(ConsoleKind::I2pd, page),
        Some("2.59.0".to_owned())
    );
    let first = "<b>A:</b> 1.2.3<br><b>B:</b> 4.5.6<br>";
    assert_eq!(
        console_version(ConsoleKind::I2pd, first),
        Some("1.2.3".to_owned())
    );
}

#[test]
fn r18_no_version_in_the_page_gives_none() {
    // R18: null when it is not found.
    assert_eq!(console_version(ConsoleKind::Java, JAVA_BODY), None);
    assert_eq!(console_version(ConsoleKind::Java, ""), None);
    assert_eq!(console_version(ConsoleKind::I2pd, I2PD_BODY), None);
    assert_eq!(console_version(ConsoleKind::I2pd, ""), None);
    assert_eq!(
        console_version(ConsoleKind::I2pd, "<b>Uptime:</b> soon<br>"),
        None
    );
    assert_eq!(
        console_version(ConsoleKind::I2pd, "<b>Version:</b> beta<br>"),
        None
    );
}

#[test]
fn r18_each_kind_reads_only_its_own_marker() {
    // R18: the Java rule does not read an i2pd page, and the other way round.
    let java = r#"<link href="/themes/console/light/console.css?2.13.0">"#;
    let i2pd = "<b>Version:</b> 2.59.0<br>";
    assert_eq!(console_version(ConsoleKind::I2pd, java), None);
    assert_eq!(console_version(ConsoleKind::Java, i2pd), None);
}

#[test]
fn r18_a_verified_console_carries_the_version_of_its_probe_page() {
    // R18: ConsoleInfo.version, read from the page the probe fetched.
    for kind in [ConsoleKind::Java, ConsoleKind::I2pd] {
        let fake = FakeConsole::start(kind);
        let console = fake.verified();
        assert_eq!(console.version(), Some("2.13.0"));
        assert_eq!(console.info().version.as_deref(), Some("2.13.0"));
        assert_eq!(probe(kind, fake.port()).unwrap().version(), Some("2.13.0"));
    }
}

#[test]
fn r18_the_version_costs_no_extra_request() {
    // R18: no extra request: one probe is one GET.
    for kind in [ConsoleKind::Java, ConsoleKind::I2pd] {
        let fake = FakeConsole::start(kind);
        let _console = fake.verified();
        assert_eq!(fake.requests().len(), 1, "{:?}", fake.requests());
    }
    let body = format!("{JAVA_BODY}<link href=\"/themes/console/light/console.css?2.13.0\">");
    let plain = Plain::start("200 OK", "", &body);
    assert_eq!(
        probe(ConsoleKind::Java, plain.port).unwrap().version(),
        Some("2.13.0")
    );
    assert_eq!(plain.heads().len(), 1);
}

#[test]
fn r18_a_console_without_a_version_still_counts_and_reports_null() {
    // R18: the version is display only; its absence changes nothing else.
    let java = Plain::start("200 OK", "", JAVA_BODY);
    let found = probe(ConsoleKind::Java, java.port).expect("still a console");
    assert_eq!(found.version(), None);
    assert_eq!(
        serde_json::to_value(found.info()).unwrap()["version"],
        json!(null)
    );
    let i2pd = Plain::start("200 OK", "", I2PD_BODY);
    let found = probe(ConsoleKind::I2pd, i2pd.port).expect("still a console");
    assert_eq!(found.version(), None);
    assert_eq!(found.pages().len(), 3);
}

#[test]
fn r18_the_version_does_not_change_the_pages_or_the_origin() {
    // R18: it changes nothing else.
    let fake = FakeConsole::start(ConsoleKind::Java);
    let info = fake.verified().info();
    assert_eq!(info.pages, ConsolePage::ALL);
    assert_eq!(
        info.origin,
        Some(format!("http://127.0.0.1:{}", fake.port()))
    );
    assert!(info.found);
}

// ---------------------------------------------------------------- R9, R10: route

fn route_of(kind: ConsoleKind, target: &str) -> ConsoleNav {
    let fake = FakeConsole::start(kind);
    fake.verified().route(&url(target))
}

fn consoles() -> Vec<(FakeConsole, crate::net::console::VerifiedConsole)> {
    [ConsoleKind::Java, ConsoleKind::I2pd]
        .into_iter()
        .map(|kind| {
            let fake = FakeConsole::start(kind);
            let console = fake.verified();
            (fake, console)
        })
        .collect()
}

#[test]
fn r10_the_detected_origin_stays_in_the_console_view() {
    // R9 and R10: same origin, any path, query and fragment: Stay.
    for (_fake, console) in consoles() {
        let origin = console.origin();
        for tail in [
            "",
            "/",
            "/home",
            "/config?x=1",
            "/?page=tunnels",
            "/dns#top",
            "/a/b/c.css",
        ] {
            let target = url(&format!("{origin}{tail}"));
            assert_eq!(console.route(&target), ConsoleNav::Stay, "{target}");
        }
    }
}

#[test]
fn r10_every_page_of_the_router_stays() {
    // R7 and R10: the page URLs are on the origin.
    for (_fake, console) in consoles() {
        for page in console.pages() {
            let target = console.url(page).unwrap();
            assert_eq!(console.route(&target), ConsoleNav::Stay, "{target}");
        }
    }
}

#[test]
fn r10_i2p_sites_open_in_a_new_tab() {
    // R10: http(s)://*.i2p opens a tab.
    for target in [
        "http://stats.i2p/",
        "https://stats.i2p/path?q=1",
        "http://sub.example.i2p/a#b",
        "HTTP://STATS.I2P/",
        "http://tc73n4kivdroccekirco7rhgxdg5f3cjvbaapabupeyzrqwv5guq.b32.i2p/",
    ] {
        assert_eq!(
            route_of(ConsoleKind::Java, target),
            ConsoleNav::OpenTab,
            "{target}"
        );
        assert_eq!(
            route_of(ConsoleKind::I2pd, target),
            ConsoleNav::OpenTab,
            "{target}"
        );
    }
}

#[test]
fn r10_other_loopback_ports_are_cancelled() {
    // R9 and R10: not the detected port.
    for (fake, console) in consoles() {
        let other = if fake.port() == 80 { 81 } else { 80 };
        let twin = if fake.port() == 7657 { 7070 } else { 7657 };
        for port in [other, twin] {
            let target = url(&format!("http://127.0.0.1:{port}/"));
            assert_eq!(console.route(&target), ConsoleNav::Cancel, "{target}");
        }
        assert_eq!(console.route(&url("http://127.0.0.1/")), ConsoleNav::Cancel);
    }
}

#[test]
fn r10_other_loopback_names_and_schemes_are_cancelled() {
    // R9: scheme http, host 127.0.0.1, the detected port, nothing else.
    for (fake, console) in consoles() {
        let port = fake.port();
        for target in [
            format!("https://127.0.0.1:{port}/"),
            format!("http://localhost:{port}/"),
            format!("http://127.0.0.2:{port}/"),
            format!("http://[::1]:{port}/"),
            format!("http://0.0.0.0:{port}/"),
            format!("ws://127.0.0.1:{port}/"),
            format!("http://127.0.0.1:{port}@evil.example/"),
        ] {
            assert_eq!(console.route(&url(&target)), ConsoleNav::Cancel, "{target}");
        }
    }
}

#[test]
fn r10_clearnet_and_special_schemes_are_cancelled() {
    // R10: anything else is cancelled: clearnet, file:, data:, javascript:, and more.
    for target in [
        "http://example.com/",
        "https://example.com/",
        "http://example.i2p.evil.example/",
        "http://evil.example/?next=http://a.i2p/",
        "file:///etc/passwd",
        "data:text/html,<h1>x</h1>",
        "javascript:alert(1)",
        "about:blank",
        "blob:http://127.0.0.1:7657/abc",
        "ftp://stats.i2p/",
        "tauri://localhost/",
        "mailto:a@b.i2p",
    ] {
        assert_eq!(
            route_of(ConsoleKind::Java, target),
            ConsoleNav::Cancel,
            "{target}"
        );
        assert_eq!(
            route_of(ConsoleKind::I2pd, target),
            ConsoleNav::Cancel,
            "{target}"
        );
    }
}

// ---------------------------------------------------------------- R9, R10: property

const SCHEMES: [&str; 8] = [
    "http",
    "https",
    "ws",
    "ftp",
    "file",
    "data",
    "javascript",
    "tauri",
];
const HOSTS: [&str; 10] = [
    "127.0.0.1",
    "127.0.0.2",
    "localhost",
    "[::1]",
    "0.0.0.0",
    "2130706433",
    "foo.i2p",
    "example.com",
    "127.0.0.1.i2p",
    "evil.example",
];

fn url_text(port: u16) -> impl Strategy<Value = String> {
    let ports = prop_oneof![
        Just(String::new()),
        Just(format!(":{port}")),
        (1u16..=65535).prop_map(|p| format!(":{p}")),
    ];
    (
        proptest::sample::select(SCHEMES.to_vec()),
        proptest::sample::select(HOSTS.to_vec()),
        ports,
        "(/[a-z0-9_.-]{0,8}){0,3}(\\?[a-z]=[0-9])?(#[a-z]{0,3})?",
    )
        .prop_map(|(s, h, p, t)| format!("{s}://{h}{p}{t}"))
}

#[test]
fn r10_property_structured_urls_stay_only_on_the_exact_origin() {
    // R9 and R10: for random URLs, route() is Stay exactly for the detected origin.
    let fake = FakeConsole::start(ConsoleKind::Java);
    let console = fake.verified();
    let port = console.port();
    let base = url(&console.origin());
    let mut runner = TestRunner::new(Config::with_cases(3000));
    let result = runner.run(&url_text(port), |text| {
        if let Ok(target) = Url::parse(&text) {
            let same = target.origin() == base.origin();
            let stay = console.route(&target) == ConsoleNav::Stay;
            prop_assert_eq!(stay, same, "{}", text);
        }
        Ok(())
    });
    result.unwrap();
}

#[test]
fn r10_property_arbitrary_urls_never_stay_off_the_origin() {
    // R10: no random text makes route() Stay on another origin.
    let fake = FakeConsole::start(ConsoleKind::I2pd);
    let console = fake.verified();
    let base = url(&console.origin());
    let text = prop_oneof![
        "\\PC{0,40}",
        "[a-z]{2,10}:[ -~]{0,40}",
        "https?://[ -~]{0,40}"
    ];
    let mut runner = TestRunner::new(Config::with_cases(3000));
    let result = runner.run(&text, |text| {
        if let Ok(target) = Url::parse(&text) {
            let stay = console.route(&target) == ConsoleNav::Stay;
            prop_assert!(!stay || target.origin() == base.origin(), "{}", text);
        }
        Ok(())
    });
    result.unwrap();
}

// ---------------------------------------------------------------- R6: on demand

/// Tests that probe the default console ports run one at a time: they listen on 7657 and
/// 7070, and any other detection would count as a connection.
static DETECT_LOCK: Mutex<()> = Mutex::new(());

fn detect_lock() -> MutexGuard<'static, ()> {
    DETECT_LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A listener on a fixed loopback port that counts connections. While `up` it answers
/// with the console marker, else with 404.
struct Spy {
    hits: Arc<AtomicUsize>,
    up: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Spy {
    /// `None` when the port is taken, for example by a real router.
    fn on(port: u16, body: &'static str) -> Option<Spy> {
        let listener = TcpListener::bind(("127.0.0.1", port)).ok()?;
        listener.set_nonblocking(true).ok()?;
        let hits = Arc::new(AtomicUsize::new(0));
        let up = Arc::new(AtomicBool::new(true));
        let stop = Arc::new(AtomicBool::new(false));
        let (h, u, s) = (Arc::clone(&hits), Arc::clone(&up), Arc::clone(&stop));
        let thread = thread::spawn(move || spy_loop(&listener, &h, &u, &s, body));
        Some(Spy {
            hits,
            up,
            stop,
            thread: Some(thread),
        })
    }

    fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }

    fn set_up(&self, up: bool) {
        self.up.store(up, Ordering::SeqCst);
    }
}

impl Drop for Spy {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn spy_reply(up: bool, body: &str) -> String {
    let (status, text) = if up {
        ("200 OK", body)
    } else {
        ("404 Not Found", "")
    };
    format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
        text.len()
    )
}

fn spy_loop(
    listener: &TcpListener,
    hits: &AtomicUsize,
    up: &AtomicBool,
    stop: &AtomicBool,
    body: &str,
) {
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                hits.fetch_add(1, Ordering::SeqCst);
                stream.set_nonblocking(false).unwrap();
                let reply = spy_reply(up.load(Ordering::SeqCst), body);
                reply_to(stream, &reply, &Mutex::new(Vec::new()));
            }
            Err(_) => thread::sleep(Duration::from_millis(10)),
        }
    }
}

/// Listeners on both default ports. `None` when a port is taken, for example by a real
/// router on a dev machine: the test then skips (it fails on CI, where the ports are free).
fn default_spies() -> Option<(Spy, Spy)> {
    let spies = Spy::on(JAVA_DEFAULT_PORT, JAVA_BODY).zip(Spy::on(I2PD_DEFAULT_PORT, I2PD_BODY));
    if spies.is_none() {
        assert!(
            std::env::var_os("CI").is_none(),
            "ports 7657 and 7070 must be free on CI"
        );
    }
    spies
}

/// An app on the mock runtime with the shared state, no window.
fn mock_app() -> App<MockRuntime> {
    let app = mock_builder().build(mock_context(noop_assets())).unwrap();
    app.manage(Shared::<MockRuntime>::new(Core::new(
        None,
        "127.0.0.1:4444",
        0,
    )));
    app
}

/// Calls `shell::console::stop` when dropped, also when the test fails, so the re-check
/// and retry loops of one test never keep probing the ports during the next one.
struct StopOnDrop(AppHandle<MockRuntime>);

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        stop(&self.0);
    }
}

fn stop_on_drop(handle: &AppHandle<MockRuntime>) -> StopOnDrop {
    StopOnDrop(handle.clone())
}

/// The `console-changed` payloads of `handle`.
fn changes(handle: &AppHandle<MockRuntime>) -> Receiver<String> {
    let (tx, rx) = channel();
    handle.listen("console-changed", move |event| {
        let _ = tx.send(event.payload().to_owned());
    });
    rx
}

fn wait_until(seconds: u64, mut done: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(seconds);
    while !done() && Instant::now() < end {
        thread::sleep(Duration::from_millis(50));
    }
    done()
}

#[test]
fn r6_start_up_opens_no_connection_to_a_console_port() {
    // R6: eepview never probes at start (the leak test must open no extra socket).
    let _guard = detect_lock();
    let Some((java, i2pd)) = default_spies() else {
        return;
    };
    let mut app = mock_app();
    let _stop = stop_on_drop(app.handle());
    crate::shell::chrome::build(&mut app).unwrap();
    crate::shell::watch::start(app.handle(), Err("EEPVIEW_PROXY: test".to_owned()));
    thread::sleep(Duration::from_secs(12));
    assert_eq!(
        (java.hits(), i2pd.hits()),
        (0, 0),
        "no probe at start, nor after 10 s"
    );
    assert!(current(app.handle()).is_none());
}

#[test]
fn r6_detect_here_probes_the_candidates_of_this_machine() {
    // R5 and R6: detect_here is detect over the candidates of the process environment.
    let _guard = detect_lock();
    let Some((java, _i2pd)) = default_spies() else {
        return;
    };
    let found = detect_here().expect("the console on the default port");
    assert_eq!(
        (found.kind(), found.port()),
        (ConsoleKind::Java, JAVA_DEFAULT_PORT)
    );
    assert!(java.hits() >= 1);
}

#[test]
fn r6_detect_now_probes_stores_emits_and_answers_the_info() {
    // R6: detection runs on demand, stores the result and emits console-changed.
    let _guard = detect_lock();
    let Some((java, _i2pd)) = default_spies() else {
        return;
    };
    let app = mock_app();
    let _stop = stop_on_drop(app.handle());
    let events = changes(app.handle());
    let info = detect_now(app.handle());
    assert!(java.hits() >= 1, "detect_now probes");
    assert!(info.found);
    assert_eq!(info.kind, Some(ConsoleKind::Java));
    assert_eq!(info.origin.as_deref(), Some("http://127.0.0.1:7657"));
    assert_eq!(current(app.handle()).map(|c| c.info()), Some(info.clone()));
    let payload = events
        .recv_timeout(Duration::from_secs(10))
        .expect("console-changed");
    let sent: serde_json::Value = serde_json::from_str(&payload).unwrap();
    assert_eq!(sent, serde_json::to_value(&info).unwrap());
}

#[test]
fn r6_detect_now_with_nothing_listening_answers_no_console() {
    // R6 and R5: no candidate passes, so no console is stored.
    let _guard = detect_lock();
    let Some((java, i2pd)) = default_spies() else {
        return;
    };
    java.set_up(false);
    i2pd.set_up(false);
    let app = mock_app();
    let _stop = stop_on_drop(app.handle());
    let info = detect_now(app.handle());
    assert_eq!(info, ConsoleInfo::none());
    assert!(current(app.handle()).is_none());
}

#[test]
fn r6_detect_now_replaces_a_stored_console_and_closes_its_window() {
    // R6: a console that goes away closes the console window.
    let _guard = detect_lock();
    let Some((java, i2pd)) = default_spies() else {
        return;
    };
    java.set_up(false);
    i2pd.set_up(false);
    let app = mock_app();
    let _stop = stop_on_drop(app.handle());
    let fake = FakeConsole::start(ConsoleKind::Java);
    let old = fake.verified();
    set_console(app.handle(), Some(old.clone()));
    ConsoleWebview::open(app.handle(), &old, ConsolePage::Home).unwrap();
    assert!(app.get_window("console-window").is_some());
    let info = detect_now(app.handle());
    assert!(!info.found);
    assert!(current(app.handle()).is_none());
    assert!(wait_until(10, || app.get_webview("console").is_none()));
}

#[test]
fn r21_a_known_console_survives_two_misses_and_is_cleared_by_the_third() {
    // R6 and R21: re-check every 10 s; the console stays through 2 misses, no event; the
    // third miss clears it with console-changed found:false.
    let _guard = detect_lock();
    let Some((java, i2pd)) = default_spies() else {
        return;
    };
    let app = mock_app();
    let _stop = stop_on_drop(app.handle());
    let events = changes(app.handle());
    assert!(detect_now(app.handle()).found);
    let after_first = java.hits();
    // Both consoles go down: a re-check that found the other one would replace the known
    // console at once (R21) and the misses would never count.
    java.set_up(false);
    i2pd.set_up(false);
    assert!(
        wait_until(30, || java.hits() >= after_first + 2),
        "two re-checks ran"
    );
    assert!(
        current(app.handle()).is_some(),
        "two misses keep the console"
    );
    let seen: Vec<String> = events.try_iter().collect();
    assert!(
        seen.iter().all(|e| !e.contains("\"found\":false")),
        "no event yet: {seen:?}"
    );
    assert!(
        wait_until(30, || current(app.handle()).is_none()),
        "the third miss clears it"
    );
    let last = events.try_iter().last().expect("console-changed");
    assert!(last.contains("\"found\":false"), "{last}");
}

#[test]
fn r21_a_re_check_that_finds_the_console_again_resets_the_count() {
    // R21: miss, then found, then misses again: the count restarted. Only the Java
    // console exists here, so no re-check finds another console.
    let _guard = detect_lock();
    let Some((java, i2pd)) = default_spies() else {
        return;
    };
    i2pd.set_up(false);
    let app = mock_app();
    let _stop = stop_on_drop(app.handle());
    assert!(detect_now(app.handle()).found);
    let start = java.hits();
    java.set_up(false);
    assert!(wait_until(30, || java.hits() >= start + 2));
    java.set_up(true);
    let mid = java.hits();
    assert!(wait_until(30, || java.hits() > mid));
    java.set_up(false);
    let again = java.hits();
    assert!(wait_until(30, || java.hits() >= again + 2));
    assert!(
        current(app.handle()).is_some(),
        "two misses after a find keep it"
    );
}

#[test]
fn r20_no_console_found_retries_and_stops_at_the_first_console() {
    // R20: a miss retries every 10 s; the first console found ends it.
    let _guard = detect_lock();
    let Some((java, i2pd)) = default_spies() else {
        return;
    };
    java.set_up(false);
    i2pd.set_up(false);
    let app = mock_app();
    let _stop = stop_on_drop(app.handle());
    let events = changes(app.handle());
    assert!(!detect_now(app.handle()).found);
    let first = java.hits();
    java.set_up(true);
    assert!(
        wait_until(30, || current(app.handle()).is_some()),
        "a retry found it"
    );
    assert!(java.hits() > first);
    let sent = events
        .recv_timeout(Duration::from_secs(5))
        .expect("console-changed");
    assert!(sent.contains("\"found\":true"), "{sent}");
}

#[test]
fn r20_a_new_trigger_during_the_retries_starts_no_second_loop() {
    // R20: at most one retry loop. Three triggers in a row, then 25 s: one loop adds
    // about 2 probes per port, three loops would add about 6.
    let _guard = detect_lock();
    let Some((java, i2pd)) = default_spies() else {
        return;
    };
    java.set_up(false);
    i2pd.set_up(false);
    let app = mock_app();
    let _stop = stop_on_drop(app.handle());
    for _ in 0..3 {
        assert!(!detect_now(app.handle()).found);
    }
    let after_triggers = java.hits();
    assert!(after_triggers >= 3, "each trigger probes");
    thread::sleep(Duration::from_secs(25));
    let retries = java.hits() - after_triggers;
    assert!(
        (1..=3).contains(&retries),
        "one loop retries 2 times in 25 s, saw {retries}"
    );
}

#[test]
fn r22_stop_ends_the_re_check_loop_and_a_later_detect_now_starts_it_again() {
    // R22: after stop no thread opens a connection to a console port, at the latest one
    // tick (10 s) later. A later detect_now starts the loops again.
    let _guard = detect_lock();
    let Some((java, i2pd)) = default_spies() else {
        return;
    };
    let app = mock_app();
    let _stop = stop_on_drop(app.handle());
    assert!(detect_now(app.handle()).found);
    let looping = java.hits();
    assert!(
        wait_until(30, || java.hits() > looping),
        "the re-check loop probes the known console"
    );
    stop(app.handle());
    thread::sleep(Duration::from_secs(11));
    let settled = (java.hits(), i2pd.hits());
    thread::sleep(Duration::from_secs(11));
    assert_eq!(
        (java.hits(), i2pd.hits()),
        settled,
        "no connection to a console port after stop and one tick"
    );
    assert!(detect_now(app.handle()).found);
    let restarted = java.hits();
    assert!(
        wait_until(30, || java.hits() > restarted),
        "a later detect_now starts the re-check loop again"
    );
}

#[test]
fn r22_stop_ends_the_retry_loop_and_a_later_detect_now_starts_it_again() {
    // R22: the retry loop (nothing found) ends too: after stop and one tick no connection
    // to a console port opens. A later detect_now starts the retries again.
    let _guard = detect_lock();
    let Some((java, i2pd)) = default_spies() else {
        return;
    };
    java.set_up(false);
    i2pd.set_up(false);
    let app = mock_app();
    let _stop = stop_on_drop(app.handle());
    assert!(!detect_now(app.handle()).found);
    let looping = java.hits();
    assert!(
        wait_until(30, || java.hits() > looping),
        "the retry loop probes again"
    );
    stop(app.handle());
    thread::sleep(Duration::from_secs(11));
    let settled = (java.hits(), i2pd.hits());
    thread::sleep(Duration::from_secs(11));
    assert_eq!(
        (java.hits(), i2pd.hits()),
        settled,
        "no connection to a console port after stop and one tick"
    );
    assert!(!detect_now(app.handle()).found);
    let restarted = java.hits();
    assert!(
        wait_until(30, || java.hits() > restarted),
        "a later detect_now starts the retry loop again"
    );
}

// ---------------------------------------------------------------- R2 (clarified): u16 tokens

fn java_args(args: &str) -> String {
    format!("clientApp.0.main=net.i2p.router.web.RouterConsoleRunner\nclientApp.0.args={args}\n")
}

#[test]
fn r2_java_port_is_the_first_token_that_parses_as_a_non_zero_u16() {
    // R2: the first whitespace-separated token that parses fully as a non-zero u16.
    assert_eq!(
        java_console_port(&java_args("7657 ::1,127.0.0.1 ./webapps/")),
        Some(7657)
    );
    assert_eq!(
        java_console_port(&java_args("0 7658 ./webapps/")),
        Some(7658)
    );
    assert_eq!(
        java_console_port(&java_args("65536 7659 ./webapps/")),
        Some(7659)
    );
    assert_eq!(java_console_port(&java_args("7660abc 7661")), Some(7661));
    assert_eq!(
        java_console_port(&java_args("::1,127.0.0.1 7662")),
        Some(7662)
    );
}

#[test]
fn r2_java_tls_only_args_give_no_port() {
    // R2: -s 7667 ::1,127.0.0.1 ./webapps/ gives no port: 127.0.0.1 is not a u16 token.
    assert_eq!(
        java_console_port(&java_args("-s 7667 ::1,127.0.0.1 ./webapps/")),
        None
    );
    assert_eq!(java_console_port(&java_args("99999 ./webapps/")), None);
    assert_eq!(java_console_port(&java_args("0 ./webapps/")), None);
    assert_eq!(java_console_port(&java_args("")), None);
}

// ---------------------------------------------------------------- R4 (clarified)

#[test]
fn r4_probe_sends_one_http_1_0_get_in_origin_form() {
    // R4: one `GET <path> HTTP/1.0`, origin form, so the answer is never chunked.
    for (kind, path) in [(ConsoleKind::Java, "/home"), (ConsoleKind::I2pd, "/")] {
        let body = if kind == ConsoleKind::Java {
            JAVA_BODY
        } else {
            I2PD_BODY
        };
        let plain = Plain::start("200 OK", "", body);
        assert!(probe(kind, plain.port).is_some());
        let heads = plain.heads();
        assert_eq!(heads.len(), 1, "one request");
        let line = heads[0].lines().next().unwrap();
        assert_eq!(line, format!("GET {path} HTTP/1.0"));
    }
}

#[test]
fn r4_the_fake_console_sees_origin_form_get_lines_only() {
    // R4: only GET, in origin form (no absolute URL).
    let fake = FakeConsole::start(ConsoleKind::Java);
    let _console = fake.verified();
    for line in fake.requests() {
        assert!(line.starts_with("GET /"), "{line}");
        assert!(!line.contains("http://"), "{line}");
    }
}

#[test]
fn r4_probe_gives_up_after_about_five_seconds_on_a_silent_server() {
    // R4: a 5 s timeout.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let holder = thread::spawn(move || {
        let held = listener.accept().map(|(stream, _)| stream);
        thread::sleep(Duration::from_secs(9));
        drop(held);
    });
    let start = Instant::now();
    assert!(probe(ConsoleKind::Java, port).is_none());
    let took = start.elapsed();
    assert!(took >= Duration::from_secs(4), "gave up after {took:?}");
    assert!(took <= Duration::from_secs(8), "gave up after {took:?}");
    holder.join().unwrap();
}

#[test]
fn r4_wire_names_are_lower_case() {
    // R7 and R4 interface: kinds and pages serialize in lower case.
    let java = FakeConsole::start(ConsoleKind::Java).verified().info();
    let value = serde_json::to_value(&java).unwrap();
    assert_eq!(value["kind"], json!("java"));
    assert_eq!(
        value["pages"],
        json!(["home", "tunnels", "addressbook", "config", "logs"])
    );
}

// ---------------------------------------------------------------- R19: rule list

const OTHER_URLS: [&str; 14] = [
    "http://stats.i2p/",
    "https://stats.i2p/path",
    "http://example.com/",
    "https://example.com/",
    "http://localhost/",
    "file:///etc/passwd",
    "javascript:alert(1)",
    "ftp://127.0.0.1/",
    "ws://127.0.0.1:1/",
    "http://127.0.0.2/",
    "http://[::1]/",
    "tauri://localhost/",
    "chrome://version/",
    "view-source:http://127.0.0.1/",
];

#[test]
fn r19_engine_allows_the_console_origin() {
    // R19: URLs on the console origin (R9) pass.
    for (fake, console) in consoles() {
        let base = format!("http://127.0.0.1:{}", fake.port());
        for tail in [
            "/",
            "/home",
            "/config?x=1",
            "/?page=tunnels",
            "/themes/console/light/console.css?2.13.0",
            "/a/b#c",
        ] {
            assert!(
                console.engine_allows(&format!("{base}{tail}")),
                "{base}{tail}"
            );
        }
    }
}

#[test]
fn r19_engine_allows_about_data_and_blob() {
    // R19: `about:`, `data:`, `blob:` pass.
    for (_fake, console) in consoles() {
        for url in [
            "about:blank",
            "about:srcdoc",
            "data:text/html,<h1>x</h1>",
            "data:image/png;base64,AAAA",
            "blob:http://127.0.0.1:1/abc-def",
        ] {
            assert!(console.engine_allows(url), "{url}");
        }
    }
}

#[test]
fn r19_engine_blocks_everything_else() {
    // R19: nothing else, including .i2p and clearnet.
    for (_fake, console) in consoles() {
        for url in OTHER_URLS {
            assert!(!console.engine_allows(url), "{url}");
        }
        assert!(!console.engine_allows(""));
    }
}

#[test]
fn r19_engine_blocks_other_loopback_ports_and_lookalikes() {
    // R19: other loopback ports, port prefixes and host tricks are blocked.
    for (fake, console) in consoles() {
        let port = fake.port();
        let other = if port == 80 { 81 } else { 80 };
        let twin = if port == 7657 { 7070 } else { 7657 };
        for url in [
            format!("http://127.0.0.1:{other}/"),
            format!("http://127.0.0.1:{twin}/"),
            format!("http://127.0.0.1:{port}0/"),
            format!("http://127.0.0.1:{port}.evil.example/"),
            format!("http://127.0.0.1:{port}@evil.example/"),
            format!("http://localhost:{port}/"),
            format!("http://127.0.0.2:{port}/"),
            format!("http://[::1]:{port}/"),
            format!("https://127.0.0.1:{port}/"),
            format!("http://evil.example/?u=http://127.0.0.1:{port}/"),
            "http://127.0.0.1/".to_owned(),
        ] {
            assert!(!console.engine_allows(&url), "{url}");
        }
    }
}

fn rules_of(console: &crate::net::console::VerifiedConsole) -> Vec<serde_json::Value> {
    console
        .rule_list()
        .as_array()
        .expect("a JSON array")
        .clone()
}

#[test]
fn r19_rule_list_blocks_every_url_first() {
    // R19: block every URL, then allow.
    for (_fake, console) in consoles() {
        let rules = rules_of(&console);
        assert!(rules.len() >= 2, "{rules:?}");
        assert_eq!(rules[0]["action"]["type"], json!("block"));
        assert_eq!(rules[0]["trigger"]["url-filter"], json!(".*"));
    }
}

#[test]
fn r19_every_later_rule_only_ignores_the_block_for_allowed_urls() {
    // R19: after the block, only ignore-previous-rules entries.
    for (_fake, console) in consoles() {
        for rule in &rules_of(&console)[1..] {
            assert_eq!(
                rule["action"]["type"],
                json!("ignore-previous-rules"),
                "{rule}"
            );
            assert!(rule["trigger"]["url-filter"].is_string(), "{rule}");
        }
    }
}

#[test]
fn r19_rule_list_names_the_console_port_and_the_local_schemes_only() {
    // R19: the console origin plus about:, data:, blob:; no .i2p, no other port.
    for (fake, console) in consoles() {
        let rules = rules_of(&console);
        let filters: Vec<String> = rules[1..]
            .iter()
            .map(|r| r["trigger"]["url-filter"].as_str().unwrap().to_owned())
            .collect();
        let all = filters.join("\n");
        assert!(all.contains(&fake.port().to_string()), "{all}");
        for scheme in ["about", "data", "blob"] {
            assert!(all.contains(scheme), "{scheme} missing in {all}");
        }
        assert!(!all.contains("i2p"), "{all}");
        assert!(
            !rules_of(&console)
                .iter()
                .any(|r| r.to_string().contains("example"))
        );
        assert!(
            !console.rule_list().to_string().contains('|'),
            "no | in WebKit rules"
        );
    }
}

#[test]
fn r19_rule_list_differs_by_console_port() {
    // R19: the list is for this console's origin.
    let one = FakeConsole::start(ConsoleKind::Java);
    let two = FakeConsole::start(ConsoleKind::Java);
    assert_ne!(one.verified().rule_list(), two.verified().rule_list());
    assert!(
        !one.verified()
            .engine_allows(&format!("http://127.0.0.1:{}/", two.port()))
    );
}

const ENGINE_SCHEMES: [&str; 8] = [
    "http", "https", "ws", "ftp", "file", "about", "data", "blob",
];
const ENGINE_HOSTS: [&str; 8] = [
    "127.0.0.1",
    "127.0.0.2",
    "localhost",
    "[::1]",
    "0.0.0.0",
    "foo.i2p",
    "example.com",
    "127.0.0.1.i2p",
];

fn engine_text(port: u16) -> impl Strategy<Value = (String, String, String, String)> {
    let ports = prop_oneof![
        Just(String::new()),
        Just(format!(":{port}")),
        (1u16..=65535).prop_map(|p| format!(":{p}")),
    ];
    (
        proptest::sample::select(ENGINE_SCHEMES.to_vec()).prop_map(str::to_owned),
        proptest::sample::select(ENGINE_HOSTS.to_vec()).prop_map(str::to_owned),
        ports,
        "(/[a-z0-9_.-]{0,8}){0,3}(\\?[a-z]=[0-9])?",
    )
}

#[test]
fn r19_property_engine_allows_only_the_origin_and_local_schemes() {
    // R19: for random URLs, engine_allows is true only for the console origin and
    // about:, data:, blob:.
    let fake = FakeConsole::start(ConsoleKind::I2pd);
    let console = fake.verified();
    let port = console.port();
    let mut runner = TestRunner::new(Config::with_cases(3000));
    let result = runner.run(&engine_text(port), |(s, h, p, t)| {
        let text = format!("{s}://{h}{p}{t}");
        let local = matches!(s.as_str(), "about" | "data" | "blob");
        let origin = s == "http" && h == "127.0.0.1" && p == format!(":{port}");
        prop_assert_eq!(console.engine_allows(&text), local || origin, "{}", text);
        Ok(())
    });
    result.unwrap();
}

#[test]
fn r19_property_arbitrary_text_is_allowed_only_on_the_origin_or_local_schemes() {
    // R19: no random text passes unless it is on the origin or a local scheme.
    let fake = FakeConsole::start(ConsoleKind::Java);
    let console = fake.verified();
    let origin = console.origin();
    let text = prop_oneof![
        "\\PC{0,40}",
        "[a-z]{2,10}:[ -~]{0,40}",
        "https?://[ -~]{0,40}"
    ];
    let mut runner = TestRunner::new(Config::with_cases(3000));
    let result = runner.run(&text, |text| {
        let fine = ["about:", "data:", "blob:"]
            .iter()
            .any(|p| text.starts_with(p))
            || text.starts_with(&format!("{origin}/"))
            || text == origin;
        prop_assert!(!console.engine_allows(&text) || fine, "{}", text);
        Ok(())
    });
    result.unwrap();
}

// ---------------------------------------------------------------- R21: re-check misses

fn pair(kind_a: ConsoleKind, kind_b: ConsoleKind) -> (FakeConsole, FakeConsole) {
    (FakeConsole::start(kind_a), FakeConsole::start(kind_b))
}

#[test]
fn r21_three_misses_in_a_row_clear_the_console() {
    // R21: the constant, and the third miss clears it.
    assert_eq!(RECHECK_MISSES, 3);
    let fake = FakeConsole::start(ConsoleKind::Java);
    let known = fake.verified();
    let (kept, misses) = after_recheck(Some(&known), 0, None);
    assert_eq!((kept.as_ref(), misses), (Some(&known), 1));
    let (kept, misses) = after_recheck(kept.as_ref(), misses, None);
    assert_eq!((kept.as_ref(), misses), (Some(&known), 2));
    let (kept, _misses) = after_recheck(kept.as_ref(), misses, None);
    assert_eq!(kept, None, "the third miss clears it");
}

#[test]
fn r21_the_same_console_resets_the_count() {
    // R21: a re-check that finds the same console resets the count.
    let fake = FakeConsole::start(ConsoleKind::I2pd);
    let known = fake.verified();
    let (kept, misses) = after_recheck(Some(&known), 2, Some(fake.verified()));
    assert_eq!((kept.as_ref(), misses), (Some(&known), 0));
    let (kept, misses) = after_recheck(kept.as_ref(), misses, None);
    assert_eq!(
        (kept.as_ref(), misses),
        (Some(&known), 1),
        "the count restarted"
    );
}

#[test]
fn r21_a_different_console_replaces_at_once() {
    // R21: another port or another type replaces it at once, with a fresh count.
    for (a, b) in [
        (ConsoleKind::Java, ConsoleKind::Java),
        (ConsoleKind::Java, ConsoleKind::I2pd),
        (ConsoleKind::I2pd, ConsoleKind::Java),
    ] {
        let (old, new) = pair(a, b);
        let known = old.verified();
        for misses in [0, 1, 2] {
            let (kept, count) = after_recheck(Some(&known), misses, Some(new.verified()));
            assert_eq!(
                (kept, count),
                (Some(new.verified()), 0),
                "{a:?}->{b:?} at {misses}"
            );
        }
    }
}

#[test]
fn r21_with_no_console_known_a_find_is_taken_and_a_miss_stays_empty() {
    // R21: nothing known: a find is stored, a miss keeps it empty.
    let fake = FakeConsole::start(ConsoleKind::Java);
    let (kept, misses) = after_recheck(None, 0, Some(fake.verified()));
    assert_eq!((kept, misses), (Some(fake.verified()), 0));
    let (kept, _) = after_recheck(None, 0, None);
    assert_eq!(kept, None);
}

#[test]
fn r21_a_miss_before_the_third_keeps_the_console_whatever_the_count_started_at() {
    // R21: 0 and 1 prior misses keep it.
    let fake = FakeConsole::start(ConsoleKind::Java);
    let known = fake.verified();
    for prior in [0, 1] {
        let (kept, misses) = after_recheck(Some(&known), prior, None);
        assert_eq!(kept.as_ref(), Some(&known), "prior {prior}");
        assert_eq!(misses, prior + 1);
    }
}

/// The console known after a re-check, by the R21 rule.
fn expected_known(
    found: Option<crate::net::console::VerifiedConsole>,
    run: u32,
    before: Option<crate::net::console::VerifiedConsole>,
) -> Option<crate::net::console::VerifiedConsole> {
    match found {
        Some(console) => Some(console),
        None if run >= 3 => None,
        None => before,
    }
}

/// Replays re-checks from a known `pool[0]` and checks each step against the R21 rule.
fn replay(
    pool: &[crate::net::console::VerifiedConsole],
    steps: &[Option<usize>],
) -> Result<(), TestCaseError> {
    let mut known = Some(pool[0].clone());
    let (mut misses, mut run) = (0, 0);
    for step in steps {
        let found = step.map(|i| pool[i].clone());
        run = if found.is_some() { 0 } else { run + 1 };
        let before = known.clone();
        (known, misses) = after_recheck(known.as_ref(), misses, found.clone());
        prop_assert_eq!(&known, &expected_known(found, run, before));
    }
    Ok(())
}

#[test]
fn r21_property_it_clears_only_after_three_misses_in_a_row() {
    // R21: over any sequence of re-checks, the console is cleared exactly when 3 misses
    // came in a row, and any find replaces or confirms it at once.
    let java = FakeConsole::start(ConsoleKind::Java);
    let i2pd = FakeConsole::start(ConsoleKind::I2pd);
    let pool = [java.verified(), i2pd.verified()];
    let steps = proptest::collection::vec(proptest::option::of(0usize..2), 1..40);
    let mut runner = TestRunner::new(Config::with_cases(500));
    let result = runner.run(&steps, |steps| replay(&pool, &steps));
    result.unwrap();
}

// ------------------------------------------------ router statistics (R24, R25, R27 to R30)

/// Requirement tests for the router statistics read from the console
/// (`docs/wiki/router-console.md`, "Router statistics from the console"): the request
/// (R24, R25) and the parsers (R27 to R30), against the two fixtures of the spec.
mod stats_from_console {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    use proptest::prelude::*;
    use proptest::test_runner::{Config, TestRunner};

    use crate::net::console::{
        ConsoleKind, STATS_MAX_ANSWER, STATS_TIMEOUT, fetch_stats, parse_console_stats,
        parse_i2pd_main, parse_java_summary, probe, stats_path,
    };
    use crate::net::stats::{Bandwidth, BuildSuccess, RouterStats, Tunnels};
    use crate::net::testing::FakeConsole;

    const JAVA: &str =
        include_str!("../../../tests/fixtures/console/java-2.13.0-xhr1-summaryframe.txt");
    const I2PD: &str =
        include_str!("../../../tests/fixtures/console/i2pd-2.58.0-main-synthetic.txt");
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
                client: Some(14),
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

    /// R27, fail closed per field: the fields in `nulls` are null; the fields in `maybe` are
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

    // ------------------------------------------------------------ fixtures (R28 to R30)

    #[test]
    fn r28_the_java_fixture_gives_the_values_of_the_spec_table() {
        assert_eq!(parse_java_summary(JAVA), expected_java());
    }

    #[test]
    fn r28_java_never_gives_the_version_the_in_out_tunnels_or_the_build_success() {
        // R28: always null from Java I2P.
        let got = parse_java_summary(JAVA);
        assert_eq!(got.version, None, "version");
        assert_eq!(got.tunnels.inbound, None, "tunnels.in");
        assert_eq!(got.tunnels.out, None, "tunnels.out");
        assert_eq!(got.tunnel_build_success_percent, BuildSuccess::default());
        assert!(got.history.is_empty());
    }

    #[test]
    fn r30_the_i2pd_fixture_gives_the_values_of_the_spec_table() {
        assert_eq!(parse_i2pd_main(I2PD), expected_i2pd());
    }

    #[test]
    fn r30_i2pd_never_gives_the_version_or_the_fields_the_main_page_lacks() {
        // R30: always null from i2pd.
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
    fn r30_crlf_line_ends_give_the_same_result_for_i2pd() {
        assert_eq!(parse_i2pd_main(&crlf(I2PD)), expected_i2pd());
    }

    #[test]
    fn r30_crlf_line_ends_give_the_same_result_for_java() {
        assert_eq!(parse_java_summary(&crlf(JAVA)), expected_java());
    }

    #[test]
    fn r30_a_mix_of_lf_and_crlf_line_ends_gives_the_same_result() {
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
    fn r24_parse_console_stats_runs_the_parser_of_the_kind() {
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
    fn r27_a_page_of_the_other_router_gives_all_null() {
        // R27, R28, R30: no anchor, no field.
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
    fn r27_an_empty_body_gives_all_null() {
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

    // ------------------------------------------------------------ Java: language (R28, R29)

    #[test]
    fn r28_java_reads_rows_by_position_in_every_console_language() {
        // R28: "never reads a row label or a title, so it works in every console language".
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
    fn r29_java_uptime_with_a_translated_unit_is_null_and_the_rest_stays() {
        // R29: "Only the English units parse"; R28: the other figures work in every language.
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
    fn r29_java_uptime_units_convert_to_milliseconds() {
        // R29: unit table, and the resolution is the unit.
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
    fn r29_java_uptime_that_is_not_n_nbsp_unit_is_null() {
        // R27, R29: an integer, `&nbsp;`, an English unit; nothing else.
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
    fn r28_java_uptime_is_row_1_of_a_table_with_2_rows() {
        // R28: sb_general or sb_shortgeneral with 2 rows: row 1.
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
    fn r28_java_advanced_uptime_is_row_1_of_4_rows_and_row_2_of_5_rows() {
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
    fn r28_java_uptime_with_another_row_count_is_null() {
        // R28: "When a table has another number of rows than the rule names, every field of
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
    fn r28_java_uptime_table_ids_win_in_the_order_general_short_advanced() {
        // R28: "The first of the three ids in this order wins."
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

    // ------------------------------------------------------------ Java: bandwidth (R28)

    #[test]
    fn r28_java_bandwidth_rows_are_read_by_position() {
        // R28: row 0 is now, row 1 is 5 min; units K (x 1 000) and M (x 1 000 000).
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
    fn r28_java_bandwidth_in_mbps_is_multiplied_by_a_million() {
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
    fn r28_java_bandwidth_of_a_young_router_has_no_5_minute_figure() {
        // R28: with 2 or 3 rows, row 1 is not read: the router is younger than 6 minutes.
        for rows in [&BANDWIDTH[..2], &BANDWIDTH[..3]] {
            let got = parse_java_summary(&java_with("sb_bandwidth", rows));
            check("2 or 3 rows", &got, &expected_java(), &BW_5M, &[]);
        }
    }

    #[test]
    fn r28_java_bandwidth_with_any_other_row_count_is_null() {
        // R28: "any row count from 2 to 4"; another count nulls the whole table.
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
    fn r28_java_bandwidth_with_a_bad_unit_or_form_is_null_for_the_row() {
        // R27, R28: `A / B&nbsp;KBps` exactly; the unit is K or M with `Bps`.
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

    // R27: digits, then optionally `.` and digits. No sign, no group separator, no `,`.
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
    fn r27_java_bandwidth_value_that_is_not_a_decimal_is_null_for_that_field() {
        for bad in NOT_A_DECIMAL {
            check_bad_bandwidth_first_value(bad);
            check_bad_bandwidth_second_value(bad);
        }
    }

    #[test]
    fn r27_java_a_bad_5_minute_row_leaves_the_now_row_alone() {
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
    fn r27_java_conversion_is_exact_on_the_decimal_digits_and_a_half_rounds_up() {
        // R27: 0.5 -> 1, 1.5 -> 2, 1000.5 -> 1001, 2001.5 -> 2002 (a float gives 1000, 2001).
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

    // ------------------------------------------------------------ Java: peers (R28)

    #[test]
    fn r28_java_peers_with_5_rows_read_rows_0_3_and_4() {
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
    fn r28_java_peersadvanced_with_6_rows_reads_rows_0_3_and_4() {
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
    fn r28_java_peers_with_the_wrong_row_count_for_the_id_are_null() {
        // R28: sb_peers needs 5 rows, sb_peersadvanced needs 6.
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
    fn r27_java_active_peers_that_is_not_a_slash_pair_is_null() {
        // R28: `A / B`: A, a space, `/`, a space, B.
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
    fn r27_java_floodfills_and_known_routers_that_are_not_integers_are_null() {
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

    // ------------------------------------------------------------ Java: tunnels (R28)

    #[test]
    fn r28_java_tunnel_rows_are_exploratory_client_participating() {
        let got = parse_java_summary(&java_with("sb_tunnels", &["1", "2", "3", "4"]));
        let mut base = expected_java();
        base.tunnels.exploratory = Some(1);
        base.tunnels.client = Some(2);
        base.tunnels.participating = Some(3);
        check("positions", &got, &base, &[], &[]);
    }

    #[test]
    fn r28_java_tunnels_with_another_row_count_are_null() {
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
    fn r27_java_tunnel_counts_that_are_not_integers_are_null_one_by_one() {
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
    fn r27_java_a_zero_count_is_a_number_not_a_missing_one() {
        let got = parse_java_summary(&swap(JAVA, ">398<", ">0<"));
        assert_eq!(got.tunnels.participating, Some(0));
    }

    // ------------------------------------------------------------ Java: network status (R28)

    #[test]
    fn r28_java_network_status_classes_map_to_the_contract_names() {
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
    fn r28_java_network_status_with_another_class_is_null() {
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
    fn r28_java_network_status_reads_the_first_span_only() {
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
    fn r28_java_network_status_text_is_not_read() {
        // R28: the class decides, in every language; the text next to it does not.
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
    fn r27_java_a_missing_section_nulls_its_fields_only() {
        // R27: "A missing section gives null for its fields only."
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
    fn r27_java_an_anchor_that_is_in_the_body_twice_nulls_its_fields() {
        // R27: "When an anchor ... is in the body more than once, the fields it gives are null."
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
    fn r27_java_a_value_cut_at_the_end_of_the_body_is_null() {
        // R27, R25: a cut value has no terminator; it is null, never a shorter number.
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

    // ------------------------------------------------------------ i2pd (R30)

    fn i2pd_with(from: &str, to: &str) -> RouterStats {
        parse_i2pd_main(&swap(I2PD, from, to))
    }

    const UPTIME_TEXT: &str = "1 day, 2 hours, 3 minutes, 4 seconds";

    #[test]
    fn r30_i2pd_uptime_forms_convert_to_milliseconds() {
        // R30: days, hours and minutes are optional, seconds always there, the order fixed;
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
    fn r30_i2pd_uptime_that_does_not_match_the_form_is_null_for_both_fields() {
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
    fn r30_i2pd_uptime_needs_its_br_terminator() {
        for end in ["</div>", "<br/>", "\n", " <br>"] {
            let body = swap(I2PD, "4 seconds<br>", &format!("4 seconds{end}"));
            let got = parse_i2pd_main(&body);
            check(end, &got, &expected_i2pd(), &UPTIME, &[]);
        }
    }

    #[test]
    fn r30_i2pd_network_status_words_are_given_in_upper_case() {
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
    fn r30_i2pd_network_status_with_a_suffix_or_another_word_is_null() {
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
    fn r30_i2pd_network_status_v6_is_a_different_label() {
        // R30: `Network status` (not `Network status v6`).
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
    fn r30_i2pd_tunnel_creation_success_rate_is_an_integer_percent() {
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
    fn r30_i2pd_total_tunnel_creation_success_rate_is_not_read() {
        // R30: "Total tunnel creation success rate" is a different label.
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
    fn r30_i2pd_bandwidth_converts_kib_per_second_exactly() {
        // R27, R30: X x 1 024, exact on the decimal digits, a half rounds up.
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
    fn r30_i2pd_bandwidth_that_does_not_match_its_form_is_null_for_that_field() {
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

    // R27, R30: Routers, Floodfills and Client Tunnels end with `&nbsp;`, Transit with `<br>`.
    const I2PD_COUNTS: [(&str, &str, &str, &str); 4] = [
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
            "<b>Client Tunnels:</b> 14",
            "&nbsp;",
            "<b>Client Tunnels:</b> {}",
            "tunnels.client",
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
    fn r30_i2pd_counts_that_are_not_integers_are_null_one_by_one() {
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
    fn r30_i2pd_counts_need_their_own_terminator() {
        // Routers, Floodfills, Client Tunnels: `&nbsp;`. Transit Tunnels: `<br>`.
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

    // R30: `<b><label>:</b> `; a translated or changed label gives null for its field.
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
    fn r30_i2pd_a_label_is_the_exact_text_with_a_space_after_it() {
        for (from, to, nulls) in I2PD_LABEL_CASES {
            let got = i2pd_with(from, to);
            check(to, &got, &expected_i2pd(), nulls, &[]);
        }
    }

    #[test]
    fn r27_i2pd_a_label_that_is_in_the_body_twice_nulls_its_fields() {
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
    fn r27_i2pd_a_missing_line_nulls_its_fields_only() {
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
    fn r30_i2pd_reads_the_english_page_only() {
        // R30: "when the body has no `<html lang="en"`, every field is null."
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
    fn r30_i2pd_a_value_cut_at_the_end_of_the_body_is_null() {
        // R27, R25: a value at the end of the body has no terminator.
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

    #[test]
    fn r27_every_prefix_of_a_fixture_parses_without_a_wrong_number() {
        // R25, R27: a body cut anywhere gives null fields or the right ones.
        for (name, body, base) in [
            ("java", JAVA, expected_java()),
            ("i2pd", I2PD, expected_i2pd()),
        ] {
            for end in 0..=body.len() {
                if !body.is_char_boundary(end) {
                    continue;
                }
                let got = parse_console_stats(
                    if name == "java" {
                        ConsoleKind::Java
                    } else {
                        ConsoleKind::I2pd
                    },
                    &body[..end],
                );
                never_wrong(&format!("{name}[..{end}]"), &got, &base);
            }
        }
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
        edits.sort_by(|a, b| b.0.cmp(&a.0));
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

    #[test]
    fn r27_property_random_byte_edits_never_panic_and_never_give_a_wrong_number() {
        // R27: after random non-digit byte edits of a fixture, every non-null field equals
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
                let names: Vec<&str> = fields(&base).iter().map(|(n, _)| *n).collect();
                for ((name, got), (_, want)) in fields(&got).into_iter().zip(fields(&base)) {
                    prop_assert!(
                        got.is_none() || got == want,
                        "{what}: {name} is {got:?}, the original is {want:?} ({names:?})"
                    );
                }
                prop_assert!(got.history.is_empty());
            }
            Ok(())
        });
        result.unwrap();
    }

    #[test]
    fn r27_property_any_bytes_never_panic_a_parser() {
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

    // ------------------------------------------------------------ R24, R25: the request

    #[test]
    fn r24_the_stats_path_is_fixed_per_router() {
        assert_eq!(stats_path(ConsoleKind::Java), JAVA_PATH);
        assert_eq!(stats_path(ConsoleKind::I2pd), "/");
    }

    #[test]
    fn r24_the_stats_path_never_carries_lang_action_or_a_console_nonce() {
        // R24: a Java I2P console saves `?lang=` in the router configuration.
        for kind in [ConsoleKind::Java, ConsoleKind::I2pd] {
            let path = stats_path(kind);
            for word in ["lang", "action", "consoleNonce"] {
                assert!(!path.contains(word), "{kind:?} path {path:?} holds {word}");
            }
        }
    }

    #[test]
    fn r25_the_bounds_are_3_seconds_and_256_kib() {
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

    impl Scripted {
        fn start(on_stats: impl Fn(&mut TcpStream) + Send + Sync + 'static) -> Scripted {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let heads = Arc::new(Mutex::new(Vec::new()));
            let log = Arc::clone(&heads);
            let on_stats: Arc<OnStats> = Arc::new(on_stats);
            thread::spawn(move || {
                for stream in listener.incoming().flatten() {
                    let (log, on_stats) = (Arc::clone(&log), Arc::clone(&on_stats));
                    thread::spawn(move || script(stream, &log, on_stats.as_ref()));
                }
            });
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
    fn r24_fetch_sends_one_get_with_the_four_headers_of_r4_and_nothing_else() {
        // R24: `GET <path> HTTP/1.0` in origin form; Host, User-Agent: eepview,
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
        let want: Vec<(String, String)> =
            want.into_iter().map(|(n, v)| (n.to_owned(), v)).collect();
        assert_eq!(headers, want);
    }

    #[test]
    fn r24_the_request_line_has_no_lang_action_or_nonce() {
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
    fn r24_fetch_gives_the_parsed_java_stats_with_an_empty_history() {
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
    fn r24_fetch_gives_the_parsed_i2pd_stats() {
        let fake = FakeConsole::serving(ConsoleKind::I2pd, "/", 200, I2PD);
        let got = fetch_stats(&fake.verified());
        assert_eq!(got, Some(expected_i2pd()));
    }

    #[test]
    fn r25_fetch_gives_none_for_a_status_other_than_200() {
        // R25: "a status other than 200 counts as the console does not answer".
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
    fn r24_fetch_never_follows_a_redirect() {
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
    fn r25_fetch_gives_none_for_an_answer_that_is_not_http() {
        let server = Scripted::start(|stream| {
            let _ = stream.write_all(b"this is not http\r\n\r\n<table id=\"sb_tunnels\"></table>");
        });
        assert_eq!(fetch_stats(&server.console()), None);
    }

    #[test]
    fn r25_fetch_gives_none_when_the_connection_closes_without_an_answer() {
        let server = Scripted::start(|_stream| {});
        assert_eq!(fetch_stats(&server.console()), None);
    }

    #[test]
    fn r25_fetch_gives_none_after_3_seconds_without_any_answer() {
        // R25: "a timeout before any answer"; the server would answer after 10 s.
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
    fn r25_a_body_cut_by_the_timeout_is_parsed_and_the_cut_value_is_null() {
        // R25, R27: "A body cut by the size cap or by the timeout is still parsed".
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
    fn r25_fetch_reads_exactly_the_first_256_kib() {
        // R25: "It reads at most 256 KiB". Everything up to the cap parses; a second
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
    fn r25_a_value_cut_by_the_size_cap_is_null() {
        // R25, R27: the cap lands inside `398`; the rest of the body is never read.
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
}
