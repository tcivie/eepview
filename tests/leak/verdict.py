# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT

"""Turn the event log and the strace log of one run into result rows.

A row is (check, result, detail). LEAK and FAIL rows fail the test. pass, info and warn do not.
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass
from pathlib import Path

from servers import SITE_HOSTS, VERIFY_HOST, EventLog, is_i2p

GATING = ("LEAK", "FAIL")


@dataclass
class Run:
    """What one launch of the app was, and how it ended."""

    name: str
    expect_js: str
    counts: bool
    code: int | None = None
    strace: Path | None = None


def canary_rows(log: EventLog, run: str, has_lan: bool) -> list[tuple]:
    rows = []
    hits = log.select("canary", run)
    for name in log.canaries:
        mine = [f"{e['peer']} {e['first']!r}" for e in hits if e["canary"] == name]
        rows.append(
            (
                f"canary {name}",
                "LEAK" if mine else "pass",
                f"{len(mine)} hits {mine[:4]}",
            )
        )
    if not has_lan:
        rows.append(
            ("canary on the LAN address", "FAIL", "this host has no LAN address")
        )
    return rows


def upstream_rows(log: EventLog, run: str) -> list[tuple]:
    seen = log.select("upstream", run)
    bad = sorted(
        {
            f"{e['method']} {e['host'] or '(no host)'}"
            for e in seen
            if not is_i2p(e["host"])
        }
    )
    hosts = sorted({e["host"] for e in seen})
    return [
        (
            "upstream saw only .i2p hosts",
            "LEAK" if bad else "pass",
            str(bad[:8]) if bad else str(hosts),
        ),
        (
            "upstream VERIFY request",
            "pass" if VERIFY_HOST in hosts else "info",
            "GET http://proxy.i2p/",
        ),
    ]


def report(log: EventLog, run: str, key: str) -> str | None:
    values = [e["v"] for e in log.select("report", run, strict=True) if e["k"] == key]
    return values[-1] if values else None


def page_rows(log: EventLog, run: Run) -> list[tuple]:
    """No false pass: the page must be served AND render. The spike missed this once."""
    served = [
        e
        for e in log.select("upstream", run.name, strict=True)
        if e["host"] == SITE_HOSTS[0] and e["path"] == "/"
    ]
    loaded = report(log, run.name, "loaded")
    js = report(log, run.name, "js")
    js_ok = js == run.expect_js
    rows = [
        (
            "page served by the upstream",
            "pass" if served else "FAIL",
            f"{len(served)} request(s)",
        ),
        (
            "page rendered (beacon after the vectors)",
            "pass" if loaded else "FAIL",
            f"loaded={loaded}",
        ),
        (
            "JavaScript state",
            "pass" if js_ok else ("FAIL" if run.counts else "warn"),
            f"want {run.expect_js}, got {js}",
        ),
    ]
    if run.expect_js == "on":
        rows += script_rows(log, run.name)
    return rows


def script_rows(log: EventLog, run: str) -> list[tuple]:
    done, frame = report(log, run, "done"), report(log, run, "frame_js")
    return [
        ("script fired every vector", "pass" if done else "FAIL", f"done={done}"),
        (
            "cross-origin iframe script ran",
            "pass" if frame else "FAIL",
            f"frame_js={frame}",
        ),
        ("WebRTC main frame", *rtc_result(log, run, "")),
        ("WebRTC iframe", *rtc_result(log, run, "frame_")),
    ]


# A host or srflx candidate that shows a real address. An mDNS `.local` name hides it.
REAL_IP_CANDIDATE = re.compile(
    r"candidate:\S+ \d+ \S+ \d+ (?!\S+\.local\b)(\S+) \d+ typ (?:host|srflx)"
)


def rtc_result(log: EventLog, run: str, prefix: str) -> tuple[str, str]:
    kind = report(log, run, f"{prefix}rtc_type")
    if kind is None:
        return "FAIL", "no report from the script"
    if kind != "function":
        return "pass", f"RTCPeerConnection is {kind}"
    candidates = json.loads(report(log, run, f"{prefix}rtc_candidates") or "[]")
    real = [m.group(1) for c in candidates if (m := REAL_IP_CANDIDATE.search(c))]
    if real:
        return "LEAK", f"the page learned real addresses: {real}"
    return (
        "warn",
        f"RTCPeerConnection exists (layer L5 is off), {len(candidates)} candidates, no real IP",
    )


def process_row(run: Run) -> tuple:
    if run.code is None:
        return (
            "app exited by itself",
            "FAIL",
            "timed out: EEPVIEW_EXIT_AFTER was not honoured",
        )
    return (
        "app exited by itself",
        "pass" if run.code == 0 else "FAIL",
        f"exit code {run.code}",
    )


STRACE_LINE = re.compile(r"^(\d+)\s+(\w+)\((\d+),(.*)$")
STRACE_V4 = re.compile(r'sin_port=htons\((\d+)\), sin_addr=inet_addr\("([\d.]+)"\)')
STRACE_V6 = re.compile(r'sin6_port=htons\((\d+)\).*?inet_pton\(AF_INET6, "([^"]+)"')
# glibc reaches systemd-resolved and nscd over Unix sockets, not over port 53.
STRACE_RESOLVER = re.compile(r'sun_path="([^"]*(?:resolve|nscd)[^"]*)"')


def strace_address(rest: str) -> tuple[str, int] | None:
    for pattern in (STRACE_V4, STRACE_V6):
        if m := pattern.search(rest):
            return m.group(2), int(m.group(1))
    return None


def strace_calls(text: str) -> list[tuple[str, str, tuple[str, int] | None]]:
    """(syscall, fd, inet address or None) for every traced call."""
    calls = []
    for line in text.splitlines():
        if m := STRACE_LINE.match(line):
            calls.append((m.group(2), m.group(3), strace_address(m.group(4))))
    return calls


def listening_ports(calls: list[tuple]) -> set[int]:
    """Ports named on a socket that also called listen(): the gatekeeper's own port."""
    listen_fds = {fd for call, fd, _a in calls if call == "listen"}
    named = [
        a
        for call, fd, a in calls
        if call in ("bind", "getsockname") and fd in listen_fds
    ]
    return {a[1] for a in named if a and a[1]}


def parse_strace(text: str) -> tuple[set[int], list[tuple[str, int]], set[str]]:
    """Ports the app listens on, every inet destination it sent to, and resolver sockets."""
    calls = strace_calls(text)
    sent = [a for call, _fd, a in calls if call in SEND_CALLS and a]
    return listening_ports(calls), sent, set(STRACE_RESOLVER.findall(text))


SEND_CALLS = ("connect", "sendto", "sendmsg")


def is_loopback(address: str) -> bool:
    return address.startswith(("127.", "::ffff:127.")) or address == "::1"


def strace_row(path: Path | None, upstream: int) -> tuple:
    """Linux only: the app may reach only its own gatekeeper and the fake upstream."""
    if path is None:
        return ("strace: sockets", "info", "not traced on this OS")
    if not path.exists():
        return ("strace: sockets", "FAIL", f"{path} is missing")
    listening, sent, resolvers = parse_strace(path.read_text(errors="ignore"))
    allowed = listening | {upstream}
    bad = {
        f"{a}:{p}" for a, p in sent if p == 53 or not is_loopback(a) or p not in allowed
    }
    bad |= resolvers
    if bad:
        return (
            "strace: sockets",
            "LEAK",
            f"outside gatekeeper {sorted(listening)} + upstream: {sorted(bad)[:8]}",
        )
    return (
        "strace: sockets",
        "pass",
        f"{len(sent)} sends, only to ports {sorted(allowed)}, no DNS",
    )


def evaluate(log: EventLog, ports: dict, run: Run) -> list[tuple]:
    return [
        process_row(run),
        *page_rows(log, run),
        *canary_rows(log, run.name, "lan" in ports),
        *upstream_rows(log, run.name),
        strace_row(run.strace, ports["upstream"]),
    ]


def failures(rows: list[tuple]) -> list[tuple]:
    return [row for row in rows if row[1] in GATING]
