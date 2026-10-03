#!/usr/bin/env python3
"""Spike S1-S3 harness: run the probe app against a fake I2P proxy and canaries, report leaks.

The fake proxy answers like an I2P HTTP proxy with no outproxy, and never connects anywhere.
A loopback HTTP canary and a UDP canary record any traffic that skips the proxy.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import socket
import subprocess
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlsplit

HERE = Path(__file__).resolve().parent
PAGE = (HERE / "test_page.html").read_text()
FRAME = (HERE / "test_frame.html").read_text()

MODES = {
    "default": {"PROBE_HARDEN": "0", "PROBE_JS": "on", "PROBE_WIN_ARGS": "none"},
    "hardened": {"PROBE_HARDEN": "1", "PROBE_JS": "on", "PROBE_WIN_ARGS": "hardened"},
    "js-off": {"PROBE_HARDEN": "1", "PROBE_JS": "off", "PROBE_WIN_ARGS": "hardened"},
    "win-trap": {"PROBE_HARDEN": "1", "PROBE_JS": "on", "PROBE_WIN_ARGS": "trap"},
}


class EventLog:
    def __init__(self) -> None:
        self.events: list[dict] = []
        self.mode = "idle"
        self.lock = threading.Lock()

    def add(self, kind: str, **fields: str) -> None:
        with self.lock:
            self.events.append({"kind": kind, "mode": self.mode, **fields})

    def of(self, mode: str, kind: str) -> list[dict]:
        return [e for e in self.events if e["mode"] == mode and e["kind"] == kind]


def render(template: str, mode: str, ports: dict) -> bytes:
    text = template.replace("__MODE__", mode).replace("__CANARY__", str(ports["canary"]))
    return text.replace("__UDP__", str(ports["udp"])).encode()


def proxy_handler(log: EventLog, ports: dict):
    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_args) -> None:
            return

        def reply(self, status: int, body: bytes, ctype: str = "text/html") -> None:
            self.send_response(status)
            self.send_header("Content-Type", ctype)
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(body)

        def do_CONNECT(self) -> None:
            log.add("connect", target=self.path)
            self.reply(502, b"I2P ERROR: No outproxy found")

        def do_GET(self) -> None:
            url = urlsplit(self.path)
            host = url.hostname or self.headers.get("Host", "")
            log.add("proxy", method=self.command, host=host, path=url.path, query=url.query)
            if host.endswith("probe.i2p"):
                return self.serve_probe(url)
            return self.reply(503, b"<h1>I2P ERROR: No outproxy found</h1>")

        do_POST = do_GET
        do_HEAD = do_GET

        def serve_probe(self, url) -> None:
            mode = parse_qs(url.query).get("mode", ["manual"])[0]
            if url.path == "/report":
                log.add("report", **{k: v[0] for k, v in parse_qs(url.query).items()})
                return self.reply(204, b"")
            pages = {"/test.html": PAGE, "/frame.html": FRAME}
            if url.path in pages:
                return self.reply(200, render(pages[url.path], mode, ports))
            return self.reply(404, b"not found")

    return Handler


def canary_handler(log: EventLog):
    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_args) -> None:
            return

        def do_GET(self) -> None:
            log.add("canary", path=self.path)
            self.send_response(200)
            self.send_header("Access-Control-Allow-Origin", "*")
            self.end_headers()

    return Handler


def serve_http(handler) -> ThreadingHTTPServer:
    server = ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def serve_udp(log: EventLog) -> socket.socket:
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind(("127.0.0.1", 0))

    def loop() -> None:
        while True:
            _data, addr = sock.recvfrom(4096)
            log.add("udp", source=f"{addr[0]}:{addr[1]}")

    threading.Thread(target=loop, daemon=True).start()
    return sock


def probe_command(binary: str, mode: str, args) -> list[str]:
    cmd = [binary]
    if args.strace:
        cmd = ["strace", "-f", "-qq", "-e", "trace=connect,sendto,sendmsg", "-o", f"strace.{mode}.log", *cmd]
    if args.xvfb:
        cmd = ["xvfb-run", "-a", *cmd]
    return cmd


def run_mode(mode: str, ports: dict, log: EventLog, args) -> None:
    env = {**os.environ, **MODES[mode], "PROBE_SECONDS": str(args.seconds)}
    env["PROBE_PROXY"] = f"http://127.0.0.1:{ports['proxy']}"
    env["PROBE_PAGE"] = f"http://probe.i2p/test.html?mode={mode}"
    log.mode = mode
    try:
        subprocess.run(probe_command(args.binary, mode, args), env=env, timeout=args.seconds + 60, check=False)
    except subprocess.TimeoutExpired:
        log.add("error", detail="probe did not exit in time")
    time.sleep(1)
    log.mode = "idle"


def report_value(log: EventLog, mode: str, key: str) -> str | None:
    values = [e.get("v") for e in log.of(mode, "report") if e.get("k") == key]
    return values[-1] if values else None


def via_proxy(log: EventLog, mode: str, host_part: str) -> bool:
    hits = log.of(mode, "proxy") + log.of(mode, "connect")
    return any(host_part in (e.get("host", "") + e.get("target", "")) for e in hits)


def loopback_result(log: EventLog, mode: str, tag: str) -> tuple[str, str]:
    if any(tag in e["path"] for e in log.of(mode, "canary")):
        return "LEAK", "the request reached the loopback canary directly"
    if any(tag in e.get("path", "") for e in log.of(mode, "proxy")):
        return "pass", "sent to the proxy, which refused it"
    return "pass", "blocked by the engine (no request seen)"


LEAKY_CANDIDATE = re.compile(r"candidate:\S+ \d+ \S+ \d+ (?!\S+\.local\b)(\S+) \d+ typ (host|srflx)")


def rtc_result(log: EventLog, mode: str, prefix: str) -> tuple[str, str]:
    kind = report_value(log, mode, f"{prefix}rtc_type")
    if kind is None:
        return "info", "no report (JS off or page did not run)"
    if kind != "function":
        return "pass", f"RTCPeerConnection is {kind}"
    candidates = json.loads(report_value(log, mode, f"{prefix}rtc_candidates") or "[]")
    leaky = [m.group(1) for c in candidates if (m := LEAKY_CANDIDATE.search(c))]
    return ("LEAK", f"real IP candidates: {leaky}") if leaky else ("warn", f"API present, {len(candidates)} candidates, no real IP")


STRACE_V4 = re.compile(r'sin_port=htons\((\d+)\), sin_addr=inet_addr\("([\d.]+)"\)')
STRACE_V6 = re.compile(r'sin6_port=htons\((\d+)\).*?inet_pton\(AF_INET6, "([^"]+)"')


def strace_result(mode: str) -> tuple[str, str]:
    path = Path(f"strace.{mode}.log")
    if not path.exists():
        return "info", "not traced on this OS"
    text = path.read_text(errors="ignore")
    found = [(int(p), a) for rx in (STRACE_V4, STRACE_V6) for p, a in rx.findall(text)]
    bad = sorted({f"{a}:{p}" for p, a in found if p == 53 or not (a.startswith("127.") or a == "::1")})
    return ("LEAK", f"direct connects: {bad[:8]}") if bad else ("pass", f"{len(found)} connects, all loopback, no DNS")


def js_result(log: EventLog, mode: str) -> tuple[str, str]:
    on, off = report_value(log, mode, "js") == "on", report_value(log, mode, "js") == "off"
    want_off = MODES[mode]["PROBE_JS"] == "off"
    ok = (off and not on) if want_off else on
    return ("pass" if ok else "FAIL"), f"js={'on' if on else 'off' if off else 'no report'}"


def evaluate(log: EventLog, mode: str) -> list[tuple[str, str, str]]:
    m = mode
    rows = [
        ("page loaded via proxy", *(("pass", "") if via_proxy(log, m, "probe.i2p") else ("FAIL", "no page request"))),
        ("JavaScript state", *js_result(log, m)),
        ("http image via proxy", *(("pass", "") if via_proxy(log, m, f"img-http-{m}") else ("info", "not seen"))),
        ("https image via proxy", *(("pass", "") if via_proxy(log, m, f"img-https-{m}") else ("info", "not seen"))),
        ("clearnet IP fetch via proxy", *(("pass", "") if via_proxy(log, m, "203.0.113.7") else ("info", "not seen"))),
        ("WebSocket via proxy", *(("pass", "") if via_proxy(log, m, f"ws-{m}") else ("info", "not seen"))),
        ("loopback 127.0.0.1", *loopback_result(log, m, f"lb-ip-{m}")),
        ("loopback localhost", *loopback_result(log, m, f"lb-name-{m}")),
        ("WebRTC main frame", *rtc_result(log, m, "")),
        ("WebRTC iframe", *rtc_result(log, m, "frame_")),
        ("UDP to STUN canary", *(("LEAK", "UDP left the engine") if log.of(m, "udp") else ("pass", "no packets"))),
        ("strace connects", *strace_result(m)),
    ]
    return rows


def print_table(results: dict) -> None:
    for mode, rows in results.items():
        print(f"\n### {sys.platform} — mode `{mode}`\n\n| Check | Result | Detail |\n|---|---|---|")
        for name, result, detail in rows:
            print(f"| {name} | {result} | {detail} |")


def parse_args():
    ap = argparse.ArgumentParser()
    ap.add_argument("--binary", required=True)
    ap.add_argument("--modes", default="default,hardened,js-off")
    ap.add_argument("--seconds", type=int, default=15)
    ap.add_argument("--xvfb", action="store_true")
    ap.add_argument("--strace", action="store_true")
    ap.add_argument("--out", default="probe-results.json")
    return ap.parse_args()


def main() -> int:
    args = parse_args()
    log = EventLog()
    ports = {"canary": serve_http(canary_handler(log)).server_port, "udp": serve_udp(log).getsockname()[1]}
    ports["proxy"] = serve_http(proxy_handler(log, ports)).server_port
    modes = args.modes.split(",")
    for mode in modes:
        run_mode(mode, ports, log, args)
    results = {mode: evaluate(log, mode) for mode in modes}
    print_table(results)
    Path(args.out).write_text(json.dumps({"platform": sys.platform, "results": results, "events": log.events}, indent=2))
    strict = [r for mode in modes if mode != "default" for r in results[mode] if r[1] in ("LEAK", "FAIL")]
    return 1 if strict else 0


if __name__ == "__main__":
    sys.exit(main())
