// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The system facts a report holds, and nothing else: no locale, screen, memory or time zone.

use super::types::RouterState;

/// The router program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouterKind {
    /// Java I2P.
    JavaI2p,
    /// i2pd.
    I2pd,
    /// Not known.
    Unknown,
}

impl RouterKind {
    /// The kind named in a router helper text.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        let lower = text.to_ascii_lowercase();
        if lower.contains("i2pd") {
            Self::I2pd
        } else if lower.contains("java") || lower.contains("i2p") {
            Self::JavaI2p
        } else {
            Self::Unknown
        }
    }

    /// `Java I2P`, `i2pd` or `unknown`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        ["Java I2P", "i2pd", "unknown"][self as usize]
    }
}

/// How long eepview has run, coarsely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UptimeBucket {
    /// Under a minute.
    UnderMinute,
    /// Under 10 minutes.
    UnderTenMinutes,
    /// Under an hour.
    UnderHour,
    /// An hour or more.
    OverHour,
}

impl UptimeBucket {
    /// The bucket of `secs` seconds.
    #[must_use]
    pub fn from_secs(secs: u64) -> Self {
        match secs {
            0..60 => Self::UnderMinute,
            60..600 => Self::UnderTenMinutes,
            600..3600 => Self::UnderHour,
            _ => Self::OverHour,
        }
    }

    /// `<1 min`, `<10 min`, `<1 h` or `>1 h`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        ["<1 min", "<10 min", "<1 h", ">1 h"][self as usize]
    }
}

/// The system facts of a report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemInfo {
    /// The eepview version.
    pub version: String,
    /// The short git commit of the build.
    pub commit: String,
    /// OS family and version, for example `macOS 16.0`.
    pub os: String,
    /// CPU architecture.
    pub arch: &'static str,
    /// Web engine and version.
    pub engine: String,
    /// Router program.
    pub router_kind: RouterKind,
    /// Router version.
    pub router_version: Option<String>,
    /// Router state.
    pub router_state: RouterState,
    /// eepview runs the router.
    pub managed: bool,
    /// Page JavaScript on by default.
    pub js_default: bool,
    /// Open tabs.
    pub tabs: usize,
    /// Run time.
    pub uptime: UptimeBucket,
}

impl SystemInfo {
    /// One line per fact.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        vec![
            format!("eepview: {} ({})", self.version, self.commit),
            format!("OS: {}", self.os),
            format!("CPU: {}", self.arch),
            format!("Web engine: {}", self.engine),
            self.router_line(),
            format!(
                "JavaScript default: {}",
                if self.js_default { "on" } else { "off" }
            ),
            format!("Open tabs: {}", self.tabs),
            format!("Uptime: {}", self.uptime.as_str()),
        ]
    }

    fn router_line(&self) -> String {
        let how = if self.managed { "managed" } else { "external" };
        let version = self
            .router_version
            .as_deref()
            .map_or_else(String::new, |v| format!(" {v}"));
        format!(
            "Router: {}{version} ({how}), state {}",
            self.router_kind.as_str(),
            self.router_state.as_str()
        )
    }
}

/// A version of 1 to 5 dot-separated digit groups, 32 characters at most.
#[must_use]
pub fn clean_version(text: &str) -> Option<String> {
    let text = text.trim();
    let groups: Vec<&str> = text.split('.').collect();
    let digits = groups
        .iter()
        .all(|g| !g.is_empty() && g.bytes().all(|b| b.is_ascii_digit()));
    (text.len() <= 32 && groups.len() <= 5 && digits).then(|| text.to_owned())
}

/// `macOS <ProductVersion>` from `SystemVersion.plist`.
#[must_use]
pub fn macos_version(plist: &str) -> Option<String> {
    let after = plist.split("<key>ProductVersion</key>").nth(1)?;
    let value = after.split("<string>").nth(1)?.split("</string>").next()?;
    clean_version(value).map(|v| format!("macOS {v}"))
}

/// `Linux <ID> <VERSION_ID>` from `/etc/os-release`.
#[must_use]
pub fn linux_os(os_release: &str) -> Option<String> {
    let value = |key: &str| {
        os_release.lines().find_map(|l| {
            l.strip_prefix(key)
                .and_then(|r| r.strip_prefix('='))
                .map(|v| v.trim().trim_matches(['"', '\'']).to_owned())
        })
    };
    let id = value("ID")?;
    let id_ok = !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if !id_ok {
        return None;
    }
    let version = value("VERSION_ID").and_then(|v| clean_version(&v));
    Some(version.map_or_else(|| format!("Linux {id}"), |v| format!("Linux {id} {v}")))
}

/// `Windows <major.minor.build>` from the output of `cmd /c ver`.
#[must_use]
pub fn windows_version(ver: &str) -> Option<String> {
    let inner = ver.split("[Version ").nth(1)?.split(']').next()?;
    let parts: Vec<&str> = inner.trim().split('.').take(3).collect();
    clean_version(&parts.join(".")).map(|v| format!("Windows {v}"))
}

/// The OS family and version of this machine.
#[must_use]
pub fn os() -> String {
    os_text().unwrap_or_else(|| FAMILY.to_owned())
}

#[cfg(target_os = "macos")]
const FAMILY: &str = "macOS";
#[cfg(windows)]
const FAMILY: &str = "Windows";
#[cfg(not(any(target_os = "macos", windows)))]
const FAMILY: &str = "Linux";

#[cfg(target_os = "macos")]
fn os_text() -> Option<String> {
    let plist = std::fs::read_to_string("/System/Library/CoreServices/SystemVersion.plist").ok()?;
    macos_version(&plist)
}

#[cfg(windows)]
fn os_text() -> Option<String> {
    let mut command = std::process::Command::new("cmd");
    command.args(["/C", "ver"]);
    super::scrub::no_console(&mut command);
    let out = command.output().ok()?;
    windows_version(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(not(any(target_os = "macos", windows)))]
fn os_text() -> Option<String> {
    linux_os(&std::fs::read_to_string("/etc/os-release").ok()?)
}

/// The engine name of this system.
#[cfg(target_os = "macos")]
pub const ENGINE: &str = "WKWebView";
/// The engine name of this system.
#[cfg(windows)]
pub const ENGINE: &str = "WebView2";
/// The engine name of this system.
#[cfg(not(any(target_os = "macos", windows)))]
pub const ENGINE: &str = "WebKitGTK";

/// The web engine and its version, for example `WKWebView 621.1.15`.
#[must_use]
pub fn engine() -> String {
    engine_line(tauri::webview_version().ok().as_deref())
}

/// The engine line for a version text from the engine.
#[must_use]
pub fn engine_line(version: Option<&str>) -> String {
    version
        .and_then(clean_version)
        .map_or_else(|| ENGINE.to_owned(), |v| format!("{ENGINE} {v}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_machine_has_an_os_and_engine() {
        assert!(os().starts_with(FAMILY));
        assert!(engine().starts_with(ENGINE));
        assert_eq!(engine_line(Some("1.2")), format!("{ENGINE} 1.2"));
        assert_eq!(engine_line(Some("x")), ENGINE);
    }
}
