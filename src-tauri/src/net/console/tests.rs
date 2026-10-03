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
    candidates, console_version, detect, detect_here, i2pd_config_files, i2pd_console_port,
    i2pd_port_in, java_config_dirs, java_console_port, java_ports_in, judge, page_path, probe,
    probe_path,
};
use crate::net::testing::FakeConsole;
use crate::shell::console::{ConsoleWebview, current, detect_now, set_console};
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
        eprintln!("skipped: port 7657 or 7070 is in use");
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
fn r6_a_known_console_is_checked_again_and_none_known_is_not() {
    // R6: while a console is known it is checked every 10 s; with none known, no repeat.
    let _guard = detect_lock();
    let Some((java, _i2pd)) = default_spies() else {
        return;
    };
    let app = mock_app();
    let events = changes(app.handle());
    assert!(detect_now(app.handle()).found);
    let after_first = java.hits();
    java.set_up(false);
    assert!(
        wait_until(30, || current(app.handle()).is_none()),
        "the known console is checked again within about 10 s"
    );
    assert!(java.hits() > after_first);
    let last = events.try_iter().last().expect("console-changed");
    assert!(last.contains("\"found\":false"), "{last}");
    let settled = java.hits();
    thread::sleep(Duration::from_secs(13));
    assert_eq!(java.hits(), settled, "with none known, no repeat");
}
