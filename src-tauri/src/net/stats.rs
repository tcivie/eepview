// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Router statistics from the eepview router helper (`GET /status`, bearer token, loopback).
//!
//! The helper runs inside a managed router (spike S9). An external router has no helper, so
//! every field of [`RouterStats`] may be `null`.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use super::loopback::LoopbackAddr;
use super::verify::parse_answer;

const TIMEOUT: Duration = Duration::from_secs(3);
const MAX_ANSWER: u64 = 64 * 1024;

/// How far back [`History`] reaches: 10 minutes.
pub const HISTORY_SPAN_MS: u64 = 10 * 60 * 1000;

/// One bandwidth sample, bytes per second over the last second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Sample {
    /// Unix time in ms.
    pub t: u64,
    /// Inbound.
    #[serde(rename = "in")]
    pub inbound: u64,
    /// Outbound.
    pub out: u64,
}

/// The bandwidth of the last 10 minutes, one sample per watcher tick (5 s). Memory only.
#[derive(Debug, Default)]
pub struct History {
    samples: VecDeque<Sample>,
}

impl History {
    /// Records the 1 s bandwidth of `stats` at `now`, when the helper gave it, and drops
    /// samples older than [`HISTORY_SPAN_MS`].
    pub fn record(&mut self, now: u64, stats: &RouterStats) {
        let bw = &stats.bandwidth_bytes_per_second;
        if let (Some(inbound), Some(out)) = (bw.in1s, bw.out1s) {
            self.samples.push_back(Sample {
                t: now,
                inbound,
                out,
            });
        }
        let oldest = now.saturating_sub(HISTORY_SPAN_MS);
        while self.samples.front().is_some_and(|s| s.t < oldest) {
            self.samples.pop_front();
        }
    }

    /// The samples, oldest first.
    #[must_use]
    pub fn samples(&self) -> Vec<Sample> {
        self.samples.iter().copied().collect()
    }
}

/// Tunnel counts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Tunnels {
    /// Inbound tunnels (client + exploratory).
    #[serde(rename = "in")]
    pub inbound: Option<u64>,
    /// Outbound tunnels (client + exploratory).
    pub out: Option<u64>,
    /// Tunnels this router takes part in for others.
    pub participating: Option<u64>,
}

/// Bandwidth in bytes per second.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bandwidth {
    /// Inbound, last second.
    pub in1s: Option<u64>,
    /// Outbound, last second.
    pub out1s: Option<u64>,
    /// Inbound, 5-minute average.
    pub in5m: Option<u64>,
    /// Outbound, 5-minute average.
    pub out5m: Option<u64>,
}

/// Tunnel build success, percent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct BuildSuccess {
    /// Exploratory tunnels.
    pub exploratory: Option<u64>,
    /// Client tunnels.
    pub client: Option<u64>,
}

/// The contract `RouterStats` type. Every field is nullable.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouterStats {
    /// Router version.
    pub version: Option<String>,
    /// Uptime in ms.
    pub uptime_ms: Option<u64>,
    /// Network status name, such as `OK` or `FIREWALLED`.
    pub network_status: Option<String>,
    /// Routers in the network database.
    pub known_routers: Option<u64>,
    /// Peers with an open connection.
    pub active_peers: Option<u64>,
    /// Tunnel counts.
    pub tunnels: Tunnels,
    /// Bandwidth.
    pub bandwidth_bytes_per_second: Bandwidth,
    /// Tunnel build success.
    pub tunnel_build_success_percent: BuildSuccess,
    /// Bandwidth of the last 10 minutes at 5 s steps, oldest first.
    pub history: Vec<Sample>,
}

/// Maps the helper status JSON to [`RouterStats`].
#[must_use]
pub fn from_helper(v: &Value) -> RouterStats {
    let num = |path: &[&str]| {
        path.iter()
            .try_fold(v, |acc, k| acc.get(k))
            .and_then(Value::as_u64)
    };
    let text = |key: &str| v.get(key).and_then(Value::as_str).map(str::to_owned);
    let sum = |a: &str, b: &str| Some(num(&["tunnels", a])? + num(&["tunnels", b])?);
    RouterStats {
        version: text("version"),
        uptime_ms: num(&["uptimeMs"]),
        network_status: text("networkStatus"),
        known_routers: num(&["knownRouters"]),
        active_peers: num(&["activePeers"]),
        tunnels: Tunnels {
            inbound: sum("clientInbound", "exploratoryInbound"),
            out: sum("clientOutbound", "exploratoryOutbound"),
            participating: num(&["tunnels", "participating"]),
        },
        bandwidth_bytes_per_second: Bandwidth {
            in1s: num(&["bandwidthBytesPerSecond", "in1s"]),
            out1s: num(&["bandwidthBytesPerSecond", "out1s"]),
            in5m: num(&["bandwidthBytesPerSecond", "in5m"]),
            out5m: num(&["bandwidthBytesPerSecond", "out5m"]),
        },
        tunnel_build_success_percent: BuildSuccess {
            exploratory: num(&["tunnelBuildSuccessPercent", "exploratory"]),
            client: num(&["tunnelBuildSuccessPercent", "client"]),
        },
        history: Vec::new(),
    }
}

/// Fetches the helper status. All-null stats when the helper does not answer.
#[must_use]
pub fn fetch(addr: LoopbackAddr, token: &str) -> RouterStats {
    request(addr, token)
        .and_then(|body| serde_json::from_str::<Value>(&body).ok())
        .map(|v| from_helper(&v))
        .unwrap_or_default()
}

fn request(addr: LoopbackAddr, token: &str) -> Option<String> {
    let mut stream = addr.connect(TIMEOUT).ok()?;
    let head = format!(
        "GET /status HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {token}\r\n\
         Connection: close\r\n\r\n"
    );
    stream.write_all(head.as_bytes()).ok()?;
    let mut raw = Vec::new();
    let _ = (&mut stream).take(MAX_ANSWER).read_to_end(&mut raw);
    let (code, body) = parse_answer(&raw)?;
    (code == 200).then_some(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::thread;

    fn sample() -> Value {
        json!({
            "version": "2.13.0", "uptimeMs": 5000, "networkStatus": "OK",
            "knownRouters": 3000, "activePeers": 40,
            "tunnels": {"clientInbound": 2, "clientOutbound": 2, "exploratoryInbound": 3,
                        "exploratoryOutbound": 1, "participating": 7},
            "bandwidthBytesPerSecond": {"in1s": 10, "out1s": 20, "in15s": 1, "out15s": 2},
            "tunnelBuildSuccessPercent": {"exploratory": 50, "client": null}
        })
    }

    #[test]
    fn maps_helper_fields() {
        let s = from_helper(&sample());
        assert_eq!(s.version.as_deref(), Some("2.13.0"));
        assert_eq!(s.tunnels.inbound, Some(5));
        assert_eq!(s.tunnels.out, Some(3));
        assert_eq!(s.bandwidth_bytes_per_second.in5m, None);
        assert_eq!(s.tunnel_build_success_percent.client, None);
        let wire = serde_json::to_value(&s).unwrap();
        assert_eq!(wire["tunnels"]["in"], 5);
        assert_eq!(wire["bandwidthBytesPerSecond"]["out1s"], 20);
        assert_eq!(wire["uptimeMs"], 5000);
        assert_eq!(from_helper(&json!(null)), RouterStats::default());
    }

    #[test]
    fn history_keeps_ten_minutes() {
        let mut h = History::default();
        let stats = from_helper(&sample());
        h.record(1_000, &stats);
        h.record(1_000, &RouterStats::default());
        assert_eq!(h.samples().len(), 1);
        h.record(1_000 + HISTORY_SPAN_MS, &stats);
        assert_eq!(h.samples().len(), 2);
        h.record(2_000 + HISTORY_SPAN_MS, &stats);
        let samples = h.samples();
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].t, 1_000 + HISTORY_SPAN_MS);
        let wire = serde_json::to_value(samples[0]).unwrap();
        assert_eq!(
            wire,
            json!({"t": 1_000 + HISTORY_SPAN_MS, "in": 10, "out": 20})
        );
    }

    #[test]
    fn fetch_from_a_fake_helper() {
        let (listener, addr) = LoopbackAddr::listen_any().unwrap();
        let body = sample().to_string();
        thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];
            let n = s.read(&mut buf).unwrap();
            assert!(String::from_utf8_lossy(&buf[..n]).contains("Bearer tok"));
            let reply = format!("HTTP/1.1 200 OK\r\n\r\n{body}");
            s.write_all(reply.as_bytes()).unwrap();
        });
        assert_eq!(fetch(addr, "tok").known_routers, Some(3000));
        let (closed, dead) = LoopbackAddr::listen_any().unwrap();
        drop(closed);
        assert_eq!(fetch(dead, "tok"), RouterStats::default());
    }
}
