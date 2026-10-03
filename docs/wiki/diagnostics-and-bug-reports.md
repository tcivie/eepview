# Diagnostics and bug reports

Status: in progress in [#56](https://github.com/tcivie/eepview/pull/56).

eepview keeps a small diagnostics log on your computer. When something goes wrong, you can turn that log into a GitHub issue. The app never sends the log. You send it yourself, from your normal web browser, after you read it.

## The privacy rule

A log line or a report never holds anything that identifies you or what you visited. This holds by construction: the log takes typed values only, never text. A second layer removes anything that still looks private from the final report text.

### What is logged

- An event code, for example `router-down` or `gatekeeper-refused`.
- Typed fields: enum states (router state, refuse reason, error kind, tab kind), numbers (duration in ms, count, HTTP status code, source line), and booleans.
- The UTC time, to the second.

### What is never logged

No URL, host, path, query, page title, `.i2p` or b32 name, bookmark, history entry, search text, IP address, port of a remote peer, user name, home folder path, machine name, email address, router identity, hash or destination, cookie, header value, panic message, or free text from a page or from the OS. A refused request logs the reason kind, never the host. An error logs its kind, never its text.

## Where the files are

| System | Folder |
|---|---|
| macOS | `~/Library/Logs/io.github.tcivie.eepview/` |
| Windows | `%LOCALAPPDATA%\io.github.tcivie.eepview\logs\` |
| Linux | `~/.local/share/io.github.tcivie.eepview/logs/` |

The folder holds `eepview.log`, `eepview.1.log` and `eepview.2.log` (512 KB each at most), and a `crashed` marker after a crash. Report files go to your Downloads folder.

To delete the logs, open Settings > Privacy > Delete diagnostics logs. This also deletes the `crashed` marker. Report files you saved stay in Downloads: delete them there. Or delete the folder by hand while eepview is closed.

## How a report works

1. Open the report page. There are three ways: the toolbar menu item "Report a problem", the crash banner on the home page, and the "Report this problem" link on the blocked and router-down pages.
2. Describe the problem. Do not include site addresses.
3. Read the preview. It is exactly the text that goes out.
4. Click "Open a GitHub issue". eepview opens a prefilled issue form in your normal web browser, outside I2P. It also saves the same text to a file in Downloads and shows the file.
5. Drag the file into the GitHub issue to attach it. GitHub has no URL parameter to attach a file, so a drag is the only way.
6. Nothing is sent until you submit the issue on GitHub. GitHub sees your IP address and your GitHub account.

## Requirements

The requirement tests are written from this section. Names in `code` are the interface.

### R1 Event log (`eepview_lib::diag`)

- R1.1 `diag::event(code: Code, fields: &[Field])` is the only logging path. It records the current UTC second, the code and the fields.
- R1.2 `Field` has no `String` or `&str` variant. Its variants and their keys:

| Variant | Key | Value |
|---|---|---|
| `Field::Router(RouterState)` | `router` | `verifying`, `ok`, `down`, `not-i2p`, `paused` |
| `Field::Refuse(RefuseReason)` | `refuse` | `not-i2p`, `bad-request`, `length-required`, `upstream`, `busy` |
| `Field::Error(ErrorKind)` | `error` | see R1.4 |
| `Field::Tab(TabKind)` | `tab` | `internal`, `web` |
| `Field::Store(StoreKind)` | `store` | `bookmarks`, `history`, `settings`, `sites` |
| `Field::Op(OpKind)` | `op` | `reload`, `hard-reload`, `zoom`, `find`, `find-clear`, `back`, `forward`, `stop`, `harden`, `hover`, `first-load`, `engine-filter`, `main-thread`, `spawn`, `reveal`, `open-url`, `write-report` |
| `Field::Thread(ThreadName)` | `thread` | `main`, `router-watch`, `router-check`, `gatekeeper`, `other` |
| `Field::Source(SourceFile)` | `file` | a source file name, no folder |
| `Field::Line(u32)` | `line` | decimal |
| `Field::DurationMs(u64)` | `ms` | decimal |
| `Field::Count(u64)` | `count` | decimal |
| `Field::Status(HttpStatus)` | `status` | `500`, `502`, `503`, `504`, `other-5xx` (R12.1) |
| `Field::Managed(bool)` | `managed` | `true` or `false` |
| `Field::Js(bool)` | `js` | `true` or `false` |
| `Field::Ok(bool)` | `ok` | `true` or `false` |

- R1.3 `Code` is a closed enum. `Code::as_str()` gives the kebab-case name, and `Code::level()` gives a `Level`:

| Code | Name | Level |
|---|---|---|
| `Startup` | `startup` | `Info` |
| `Shutdown` | `shutdown` | `Info` |
| `StartFailed` | `start-failed` | `Error` |
| `Panic` | `panic` | `Error` |
| `VerifyPassed` | `verify-passed` | `Info` |
| `VerifyFailed` | `verify-failed` | `Warn` |
| `RouterUp` | `router-up` | `Info` |
| `RouterDown` | `router-down` | `Warn` |
| `GatekeeperRefused` | `gatekeeper-refused` | `Info` |
| `GatekeeperStartFailed` | `gatekeeper-start-failed` | `Error` |
| `PageLoadFailed` | `page-load-failed` | `Warn` |
| `WebviewCreateFailed` | `webview-create-failed` | `Error` |
| `EngineCallFailed` | `engine-call-failed` | `Warn` |
| `FindFailed` | `find-failed` | `Warn` |
| `ZoomFailed` | `zoom-failed` | `Warn` |
| `StoreCorrupt` | `store-corrupt` | `Warn` |
| `ThreadFailed` | `thread-failed` | `Error` |
| `ReportOpened` | `report-opened` | `Info` |
| `ReportFailed` | `report-failed` | `Warn` |

  `Level::as_str()` is `INFO`, `WARN` or `ERROR`.
- R1.4 `ErrorKind` values: `not-found`, `permission-denied`, `connection-refused`, `connection-reset`, `connection-aborted`, `broken-pipe`, `timed-out`, `addr-in-use`, `addr-not-available`, `invalid-input`, `invalid-data`, `unexpected-eof`, `write-zero`, `interrupted`, `unsupported`, `out-of-memory`, `parse`, `webview`, `platform`, `other`. `From<std::io::ErrorKind>`, `From<&std::io::Error>`, `From<&tauri::Error>` (an I/O error maps by its kind, anything else is `webview`) and `From<&serde_json::Error>` (`parse`) exist. An `io::ErrorKind` with no row is `other`.
- R1.5 Every enum in R1.2 to R1.4 has `as_str(self) -> &'static str`. `Field` implements `Display` as `key=value`.
- R1.6 `SourceFile::from_path(path: &str) -> SourceFile` keeps the part after the last `/` or `\` only when it is in `diag::SOURCE_FILES`; any other name becomes `other`. `diag::SOURCE_FILES: &[&str]` is the sorted list of the basenames of eepview's own `.rs` files (`src-tauri/src/**` and `src-tauri/crates/*/src/**`), generated by `build.rs`. `SourceFile::as_str()` returns the name or `other`. So a panic location holds a value from a closed list.
- R1.7 `ThreadName::of(name: Option<&str>) -> ThreadName` maps `main`, `router-watch`, `router-check` and `gatekeeper` to their variant, and anything else (also `None`) to `Other`.
- R1.8 `Record { at: u64, code: Code, fields: Vec<Field> }`, `at` in Unix seconds. `diag::format_line(&Record) -> String` gives `<utc> <LEVEL> <code>` and then ` key=value` per field, in order. Example: `2026-10-03T12:00:00Z INFO gatekeeper-refused refuse=not-i2p`.
- R1.9 `diag::utc(secs: u64) -> String` gives `YYYY-MM-DDTHH:MM:SSZ`. `utc(0)` is `1970-01-01T00:00:00Z`.

### R2 Storage

- R2.1 `Ring::new(capacity)`, `Ring::push(Record)`, `Ring::len()`, `Ring::is_empty()`, `Ring::last(n) -> Vec<Record>` (oldest first), `Ring::clear()`. `Ring::CAPACITY` is 2000. A full ring drops its oldest record.
- R2.2 `LogFiles::new(dir: PathBuf)` uses `LogFiles::MAX_BYTES` (524288) and `LogFiles::FILES` (3). `LogFiles::with_limit(dir, max_bytes)` sets another size. `append(&mut self, line: &str) -> io::Result<()>` writes the line and `\n` to `dir/eepview.log`, and creates `dir` when it is missing.
- R2.3 Rotation: when the current file is not empty and the line would make it larger than the limit, `eepview.1.log` becomes `eepview.2.log` (the old one is lost), `eepview.log` becomes `eepview.1.log`, and the line starts a new `eepview.log`. There are never more than 3 log files. `LogFiles::paths()` lists the files that exist, newest first. `LogFiles::delete_all()` removes them all.
- R2.4 `diag::init(log_dir: &Path)` starts writing to `LogFiles` in that folder. Events before `init` stay in the ring. `diag::recent(n) -> Vec<Record>` reads the ring. `diag::delete_logs() -> io::Result<()>` empties the ring, and deletes the files and the crash marker. No file handle stays open between two lines, so the delete also works on Windows while eepview runs.
- R2.6 `diag::default_log_dir(identifier: &str) -> Option<PathBuf>` gives the app log folder without Tauri, the same folder as `app_log_dir()`: macOS `$HOME/Library/Logs/<identifier>`, Windows `%LOCALAPPDATA%\<identifier>\logs`, Linux `$XDG_DATA_HOME/<identifier>/logs` (or `$HOME/.local/share/<identifier>/logs`). `run()` installs the panic hook and calls `init` with it before the Tauri builder, so `StartFailed` and an early panic reach the disk.
- R2.7 `LogFiles::append` writes each line with one unbuffered write, so a line is on disk before a `panic = "abort"` ends the process.
- R2.5 Nothing is ever sent. The diag module opens no socket.

### R3 Crashes

- R3.1 `diag::install_panic_hook()` sets a hook that records `Code::Panic` with `diag::panic_fields(file, line, thread)` and writes the crash marker. It never records the panic message. The hook takes the log lock with `try_lock`; when this thread already holds it, or it is poisoned and held, the hook skips the record and does not block.
- R3.2 `diag::panic_fields(file: &str, line: u32, thread: Option<&str>) -> Vec<Field>` is `[Source(SourceFile::from_path(file)), Line(line), Thread(ThreadName::of(thread))]`.
- R3.3 `diag::CRASH_MARKER` is `crashed`. `diag::write_crash_marker(dir: &Path) -> io::Result<()>` creates `dir/crashed` (empty). `diag::take_crash_marker(dir: &Path) -> bool` says whether it existed and removes it.
- R3.4 `diag::init` takes the marker. `diag::crashed_last_run() -> bool` then says whether it existed, until `diag::dismiss_crash()`.
- R3.5 On the next start, the home page shows a banner: "eepview closed unexpectedly. Report the problem?" with the buttons Report and Dismiss. Report opens `eepview://report?kind=crash`. Both buttons call `diag_crash_dismiss`.

### R4 Scrubbing

- R4.1 `diag::scrub(text: &str) -> String` is `diag::scrub_with(text, user, host)` with the current user name and machine host name.
- R4.2 `diag::scrub_with(text: &str, user: Option<&str>, host: Option<&str>) -> String` replaces each match below with `[removed]` (`diag::REMOVED`), and leaves all other text as it is:
  - a URL: `scheme://` and the text after it, up to white space or one of `"'<>`;
  - a name that ends in `.i2p` (any case), with any port, path or query after it;
  - a host name: two or more dot-separated labels whose last label is 2 to 24 letters, except a file name that ends in `.rs`, `.txt`, `.log`, `.json`, `.html`, `.ts`, `.js`, `.css`, `.md`, `.toml`, `.yml` or `.plist`;
  - a run of 52 or more base32 characters (`a-z`, `2-7`, any case);
  - a run of 44 or more base64 characters (`A-Z a-z 0-9 + / = - ~`) with both upper-case and lower-case letters: an I2P destination (about 516) or a router hash (44);
  - an IPv4 address (four numbers from 0 to 255) that is not part of a longer dotted run;
  - an IPv6 address, also inside `[ ]` and with a zone;
  - an email address;
  - `/Users/<name>`, `/home/<name>`, `<drive>:\Users\<name>` and `<drive>:/Users/<name>` (also after `\\?\`), the drive letter and the name included;
  - `user` and `host` (case does not matter, whole words only, 3 characters or more). For a host name with dots, its first label is removed too.
- R4.3 Version numbers such as `0.1.0`, `16.0`, `621.1.15.10.7` and `10.0.26100`, and times such as `12:00:00` and `2026-10-03T12:00:00Z`, stay.
- R4.4 Property: for any text, an injected URL, IPv4 address, IPv6 address, `.i2p` name, email address, home folder path or base64 destination, set apart by spaces, is not in the output.

### R5 System info (`diag::sysinfo`)

- R5.1 `SystemInfo` holds only: `version: String`, `commit: String`, `os: String`, `arch: &'static str`, `engine: String`, `router_kind: RouterKind`, `router_version: Option<String>`, `router_state: RouterState`, `managed: bool`, `js_default: bool`, `tabs: usize`, `uptime: UptimeBucket`.
- R5.2 `SystemInfo::lines(&self) -> Vec<String>`, in this order:
  - `eepview: <version> (<commit>)`
  - `OS: <os>`
  - `CPU: <arch>`
  - `Web engine: <engine>`
  - `Router: <kind> <version> (<managed|external>), state <state>` (no version: `Router: <kind> (<…>), state <state>`)
  - `JavaScript default: <on|off>`
  - `Open tabs: <n>`
  - `Uptime: <bucket>`
- R5.3 `UptimeBucket::from_secs(s)`: under 60 is `<1 min`, under 600 is `<10 min`, under 3600 is `<1 h`, else `>1 h` (`as_str`).
- R5.4 `RouterKind::from_text(text) -> RouterKind`: `i2pd` (any case) is `I2pd` (`i2pd`), `java` or `i2p` (any case) is `JavaI2p` (`Java I2P`), else `Unknown` (`unknown`).
- R5.5 `sysinfo::clean_version(text) -> Option<String>`: 1 to 5 dot-separated groups of digits, 32 characters at most, after a trim. Anything else is `None`.
- R5.6 `sysinfo::macos_version(plist: &str) -> Option<String>` reads `ProductVersion` from `SystemVersion.plist` text as `macOS <v>`. `sysinfo::linux_os(os_release: &str) -> Option<String>` gives `Linux <ID> <VERSION_ID>` (ID: `a-z 0-9 -` only, quotes removed; version: `clean_version`). `sysinfo::windows_version(ver: &str) -> Option<String>` reads `[Version 10.0.26100.1234]` as `Windows 10.0.26100`.
- R5.7 There is no locale, screen, memory size or time zone.

### R6 Report (`diag::report`)

- R6.1 `ReportKind::from_param(&str)`: `crash`, `blocked`, `router-down`, `load-failed`, anything else `General`. `ReportKind::title()`: `Problem report`, `Crash report`, `Blocked page report`, `Router down report`, `Page load report` (for `General`, `Crash`, `Blocked`, `RouterDown`, `LoadFailed`).
- R6.2 `Report { kind: ReportKind, description: String, info: SystemInfo, log: Vec<String>, include_log: bool }`. `log` holds report lines, oldest first, and then the counter lines of R12.3. The shell makes them with `diag::report_lines(&[Record]) -> Vec<String>` (R13.1) and `diag::counter_lines()` (R12.3).
- R6.3 `report::text(&Report) -> String` is the preview and the file. It is scrubbed (R4), and is:

```
What happened:
<description, or "(not given)" when it is blank>

System:
<SystemInfo::lines, one per line>

Diagnostics log (last <n> events):
<log lines>
```

  The log part is left out when `include_log` is false. `report::PREVIEW_EVENTS` is 200: the shell passes at most that many lines.
- R6.4 `report::issue_url(&Report) -> (String, bool)` gives the URL and whether the log was trimmed. The URL is `report::ISSUE_PREFIX` (`https://github.com/tcivie/eepview/issues/new`), then `?template=bug.yml&labels=bug&title=…&version=…&os=…&what-happened=…&diagnostics=…`, in that order. `version` is `<version> (<commit>)`, `os` is `<os> (<arch>)`, `what-happened` is the description, `diagnostics` is the `System:` part and the log part of R6.3. Every value is scrubbed, then encoded with `report::percent_encode`.
- R6.5 `report::percent_encode(&str) -> String` keeps `A-Z a-z 0-9 - _ . ~` and writes every other UTF-8 byte as `%XX` (upper-case hex). A space is `%20`.
- R6.6 The URL is at most `report::MAX_URL` (8000) characters. The oldest log lines go first. When any line goes, the line `report::TRIM_NOTE` (`[log trimmed, the full log is in the attached file]`) is the first line of the log part. When the URL is still too long without log lines, the description is cut: on the raw text, at a char boundary, before `percent_encode`, with `…` at the end.
- R6.7 `report::is_issue_url(&str) -> bool` is true only for `ISSUE_PREFIX` alone or followed by `?`.
- R6.8 `report::file_name(n: u32) -> String` is `eepview-report.txt` for `n` 0 or 1, and `eepview-report (<n>).txt` for `n` 2 or more. The name holds no date (R13.2).

### R6b Issue form

- R6.9 `.github/ISSUE_TEMPLATE/bug.yml` has the ids `version`, `os` (inputs), `router` (dropdown, not required), `what-happened` (textarea, required), `steps` (textarea, not required) and `diagnostics` (textarea, `render: text`, not required). So every prefilled value of R6.4 lands in a field, and no required field is left empty that the URL cannot fill.

### R7 IPC commands

Only the `internal` webview may call these (capability `capabilities/report.json`). See [IPC contract](ipc-contract.md).

| Command | Arguments | Result |
|---|---|---|
| `report_preview` | `{ kind: string, description: string, includeLog: boolean }` | `string`, the text of R6.3 |
| `report_open` | the same | `{ file: string, trimmed: boolean }`; `file` is the file name, no folder |
| `diag_crash_status` | none | `boolean`, `crashed_last_run()` |
| `diag_crash_dismiss` | none | nothing |
| `diag_logs_delete` | none | nothing; an error string on failure |

`report_open` builds the report from the last 200 events, writes `text` to `<Downloads>/<file_name>`, shows the file in the file manager, and opens `issue_url` in the system browser with `tauri-plugin-opener`. It refuses a URL for which `is_issue_url` is false, and a file outside the Downloads folder. It records `ReportOpened` or `ReportFailed`.

### R8 Capability scope

- R8.1 `capabilities/report.json` names only the `internal` webview, and grants only `allow-report-preview`, `allow-report-open`, `allow-diag-crash-status`, `allow-diag-crash-dismiss` and `allow-diag-logs-delete`.
- R8.2 No capability names these five permissions for another webview.
- R8.3 No capability grants an `opener:` permission. The plugin's own `reveal_item_in_dir` command has no scope, so JavaScript gets no opener command at all. The Rust command is the only caller, and it checks the URL prefix and the Downloads folder itself. The plugin starts with `open_js_links_on_click(false)`.
- R8.4 Only `src/shell/report.rs` calls `open_url` and `reveal_item_in_dir`.
- R8.5 No HTTP client crate and no new socket. The issue opens in the user's own browser, after a click.

### R9 Architecture rules (`src-tauri/tests/architecture.rs`)

- R9.1 `println!`, `eprintln!`, `print!`, `eprint!`, `dbg!`, `log::`, `tracing::`, `io::stdout` and `io::stderr` appear only under `src/diag/`, in `src/` and in `crates/*/src/`.
- R9.2 `console.` appears in no `.ts` file under `src/ui/` (tests included).
- R9.3 No call `diag::event(…)` has a string literal, `format!`, `to_string`, `to_owned` or `String` in its arguments.
- R9.4 R8.1 to R8.4 hold.

### R10 Where events are recorded

| Place | Event |
|---|---|
| App start, app exit | `Startup`, `Shutdown` |
| Tauri fails to start | `StartFailed` with `Error` |
| A VERIFY result that changes the router state | `VerifyPassed`, or `VerifyFailed` with `Router` |
| The router state turns ok, or stops being ok | `RouterUp`, `RouterDown` with `Router` |
| The gatekeeper refuses a request | counter `GatekeeperRefused` with `Refuse` (R12.2) |
| The gatekeeper cannot start | `GatekeeperStartFailed` with `Error` |
| The router answers 5xx, or a forwarded request fails | counter `PageLoadFailed` with `Status` or `Error` (R12.2) |
| A tab webview cannot be built | `WebviewCreateFailed` with `Error` |
| An engine call fails | `EngineCallFailed`, `FindFailed` or `ZoomFailed`, with `Op` and `Error` |
| A store file is broken | `StoreCorrupt` with `Store` and `Error` |
| A thread cannot start, or the main thread queue fails | `ThreadFailed` with `Op` and `Error` |
| A panic | `Panic` (R3) |

### R11 UI

- R11.0 `report` is in `nav::INTERNAL_PAGES`, so `eepview://report` and `src/ui/report.html` load in the `internal` webview.
- R11.1 The page `eepview://report` (`src/ui/report.html`) has a description box (empty, placeholder "What did you do, and what went wrong? Do not include site addresses."), a read-only preview, the checkbox "Include the diagnostics log" (on), the note "This opens GitHub in your normal web browser, outside I2P. GitHub sees your IP address and your GitHub account. Nothing is sent until you submit the issue on GitHub.", and the button "Open a GitHub issue". After a click it says "Drag this file into the GitHub issue to attach it."
- R11.2 `src/ui/lib/report-page.ts` exports:
  - `reportKind(search: string): "general" | "crash" | "blocked" | "router-down" | "load-failed"`;
  - `reportHref(kind): string`, which is `./report.html?kind=<kind>`;
  - `PLACEHOLDER`, `BROWSER_NOTE`, `DRAG_NOTE` and `CRASH_TEXT`, the texts of R11.1 and R3.5;
  - `openedMessage(result: { file: string; trimmed: boolean }): string`, which names the file and ends with `DRAG_NOTE`; when `trimmed`, it also says that the issue has a shorter log than the file.
- R11.3 The toolbar menu has "Report a problem" (`eepview://report`). The blocked and router-down pages have the link "Report this problem" to `reportHref("blocked")` and `reportHref("router-down")`. The link carries only the kind, never the address.
- R11.4 Settings > Privacy has the button "Delete diagnostics logs" (`id="delete-logs"`). It calls `diag_logs_delete` and says "Diagnostics logs deleted."

### R12 No value a site controls

A site can make the router answer with any status, and it can make any number of requests. So these values are a channel from the site into a public issue. They enter the log only in a closed, coarse form.

- R12.1 `HttpStatus` is a closed enum: `S500`, `S502`, `S503`, `S504`, `Other5xx` (`as_str`: `500`, `502`, `503`, `504`, `other-5xx`). `HttpStatus::from_code(code: u16) -> Option<HttpStatus>` is `None` below 500 and above 599. No field holds a raw status number.
- R12.2 Per-request results are counted, never recorded one by one. `diag::count(code: Code, field: Field)` adds 1 (saturating at `u64::MAX`) to the session counter of that pair. The gatekeeper calls it for each refusal (`Code::GatekeeperRefused`, `Field::Refuse`), each router 5xx answer (`Code::PageLoadFailed`, `Field::Status`), and each failed forwarded request (`Code::PageLoadFailed`, `Field::Error`). `diag::event` is never called for these.
- R12.3 `diag::counters() -> Vec<(Code, Field, u64)>` lists the counters above 0, sorted by the code name and then the field text. `diag::count_bucket(n: u64) -> &'static str` is `0`, `1+` (1 to 9), `10+` (10 to 99), `100+` (100 to 999) or `1000+`. `diag::counter_lines() -> Vec<String>` gives `<code> <key>=<value> count=<bucket>` per counter. A report shows only the bucket, never the exact number.
- R12.4 `diag::delete_logs` also clears the counters.

### R13 No time in a report

- R13.1 `diag::report_lines(&[Record]) -> Vec<String>` gives `<LEVEL> <code>` and ` key=value` per field, in record order. It holds no time and no time offset. The local log files keep the UTC second (R1.8).
- R13.2 The report file in Downloads is `eepview-report.txt`. When that name exists, it is `eepview-report (2).txt`, then `(3)`, up to `(99)`. `report_open` returns that name.

### R14 File safety

- R14.1 On Unix, eepview creates the log folder with mode `0700`, and sets `0700` on it when it exists. The log files, the crash marker and the report file get mode `0600`.
- R14.2 eepview never follows a symbolic link when it writes these files. The report file is created with `create_new`. The log files and the crash marker are opened with `O_NOFOLLOW` on Unix. A link in place of one of them makes the write fail.
- R14.3 `report_open` checks that the written report file lies in the Downloads folder after both paths are resolved (`canonicalize`). Otherwise it fails with `ReportFailed`.

### R15 More scrubbing (adds to R4.2)

- R15.1 Before matching, `%2E` and `%2F` (any case) become `.` and `/`.
- R15.2 A word with a dot and a non-ASCII letter is a host name (an internationalised name). The word is the run of letters, digits, `.` and `-`, with any port or path after it.
- R15.3 A run of 32 or more hex digits (`0-9 a-f A-F`) is removed.
- R15.4 Home folders: `Users` or `home` between two separators, where each separator is `/` or `\`, and the name after it up to the next separator, line end or quote (spaces are part of the name). Also `<drive>:\Documents and Settings\<name>` (either separator) and a UNC path `\<server>\<share>\<name>`, the server, share and name included.
- R15.5 `rs`, `md` and `ts` are not file endings any more (they are country domains): in free text, `apply.rs` is removed like a host name. The file endings are `txt`, `log`, `json`, `html`, `js`, `css`, `toml`, `yml` and `plist`.
- R15.6 Typed fields do not go through the free-text scrubber. `diag::scrub_report(text: &str) -> String` is `scrub`, except that a `file=<name>` token whose name is in `SOURCE_FILES` stays as it is. `report::text` and every value of `report::issue_url` use `scrub_report`. So a crash report still says where the panic happened, and the name can only be one of eepview's own source files.

### R16 Crash marker outside the log lock

- R16.1 `diag::init` keeps the log folder in a place apart from the log lock. The panic hook writes the crash marker there first, and then tries the log lock (R3.1). So the marker is written even when another thread holds the log lock.

## Limits

- The gatekeeper does not know which request is the page and which is a part of it. So the `PageLoadFailed` counter counts every failed request, and there is no separate "load failed" page yet.
- A crash in the web engine process itself is not detected yet.

## History

- 2026-10-03 — Diagnostics log and report flow — [#56](https://github.com/tcivie/eepview/pull/56)
