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
TRAP_MODES = {"win-trap"}


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
    return text.replace("__UDP__", str(ports["udp"])).replace("__LAN__", ports["lan"]).encode()


class ProbeServer(ThreadingHTTPServer):
    """A loopback HTTP server that carries the shared event log and the canary ports."""

    daemon_threads = True

    def __init__(self, handler: type[BaseHTTPRequestHandler], log: EventLog, ports: dict, host: str) -> None:
        super().__init__((host, 0), handler)
        self.log = log
        self.ports = ports


class QuietHandler(BaseHTTPRequestHandler):
    server: ProbeServer

    def log_message(self, *_args) -> None:
        return

    def log_error(self, *args) -> None:
        # Unsupported methods and broken requests land here; record them so nothing is invisible.
        self.server.log.add("http_error", detail=args[0] % args[1:], method=str(self.command))

    def reply(self, status: int, body: bytes, ctype: str = "text/html") -> None:
        self.send_response(status)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(body)


class ProxyHandler(QuietHandler):
    """Acts like an I2P HTTP proxy with no outproxy: serves probe.i2p, refuses the rest."""

    def do_CONNECT(self) -> None:
        self.server.log.add("connect", target=self.path)
        host, _, port = self.path.rpartition(":")
        if not (host.endswith("probe.i2p") and port == "80"):
            return self.reply(502, b"I2P ERROR: No outproxy found")
        # An I2P HTTP proxy tunnels CONNECT to an eepsite; WKWebView uses CONNECT even for http.
        self.send_response(200, "Connection established")
        self.end_headers()
        self.wfile.flush()
        ProxyHandler(self.connection, self.client_address, self.server)
        self.close_connection = True
        return None

    def do_GET(self) -> None:
        url = urlsplit(self.path)
        host = url.hostname or self.headers.get("Host", "").rsplit(":", 1)[0]
        self.server.log.add("proxy", method=self.command, host=host, path=url.path, query=url.query)
        if host.endswith("probe.i2p"):
            return self.serve_probe(url)
        return self.reply(503, b"<h1>I2P ERROR: No outproxy found</h1>")

    do_POST = do_GET
    do_HEAD = do_GET
    do_OPTIONS = do_GET

    def serve_probe(self, url) -> None:
        query = {k: v[0] for k, v in parse_qs(url.query).items()}
        if url.path == "/report":
            self.server.log.add("report", **query)
            return self.reply(204, b"")
        pages = {"/test.html": PAGE, "/frame.html": FRAME}
        if url.path not in pages:
            return self.reply(404, b"not found")
        return self.reply(200, render(pages[url.path], query.get("mode", "manual"), self.server.ports))


class CanaryHandler(QuietHandler):
    """Records any request that reaches a canary without the proxy."""

    def do_GET(self) -> None:
        kind = "loopback" if self.server.server_address[0] == "127.0.0.1" else "lan"
        self.server.log.add(kind, path=self.path)
        self.reply(200, b"<h1>canary</h1>")

    do_OPTIONS = do_GET


def lan_address() -> str:
    """The address of the default route. A UDP connect sends no packet."""
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
        try:
            sock.connect(("192.0.2.1", 9))
            return sock.getsockname()[0]
        except OSError:
            return "127.0.0.1"


def serve_http(handler: type[BaseHTTPRequestHandler], log: EventLog, ports: dict, host: str = "127.0.0.1") -> ProbeServer:
    server = ProbeServer(handler, log, ports, host)
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
    if mode in TRAP_MODES:
        # A trap drops the proxy, so probe.i2p cannot load; open the LAN canary to prove the bypass.
        env["PROBE_PAGE"] = f"http://{ports['lan']}/trap-{mode}"
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


def page_result(log: EventLog, mode: str) -> tuple[str, str]:
    served = [e for e in log.of(mode, "proxy") if e["host"] == "probe.i2p" and e["path"] == "/test.html"]
    if served:
        return "pass", f"{len(served)} page request(s) served"
    if via_proxy(log, mode, "probe.i2p"):
        return "FAIL", "the proxy saw the host, but the page was never requested"
    return "FAIL", "no page request"


def direct_result(log: EventLog, mode: str, tag: str, canary: str) -> tuple[str, str]:
    """Did a request tagged `tag` reach the `canary` directly, go to the proxy, or never leave?"""
    direct = [e["path"] for e in log.of(mode, canary) if tag in e["path"]]
    if direct:
        return "LEAK", f"reached the {canary} canary directly: {direct[:3]}"
    if any(tag in e.get("path", "") for e in log.of(mode, "proxy")):
        return "pass", "sent to the proxy, which refused it"
    if report_value(log, mode, "js") != "on":
        return "info", "not tested: the page script did not run"
    seen = report_value(log, mode, tag.removesuffix(f"-{mode}").replace("-", "_"))
    return "warn", f"seen at neither the proxy nor the canary; the page saw: {seen}"


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
# glibc reaches systemd-resolved and nscd over Unix sockets, not port 53.
STRACE_RESOLVER = re.compile(r'sun_path="([^"]*(?:resolve|nscd)[^"]*)"')
STRACE_V6 = re.compile(r'sin6_port=htons\((\d+)\).*?inet_pton\(AF_INET6, "([^"]+)"')


def strace_result(mode: str) -> tuple[str, str]:
    path = Path(f"strace.{mode}.log")
    if not path.exists():
        return "info", "not traced on this OS"
    text = path.read_text(errors="ignore")
    found = [(int(p), a) for rx in (STRACE_V4, STRACE_V6) for p, a in rx.findall(text)]
    bad = sorted({f"{a}:{p}" for p, a in found if p == 53 or not (a.startswith("127.") or a == "::1")})
    bad += sorted(set(STRACE_RESOLVER.findall(text)))
    return ("LEAK", f"direct connects: {bad[:8]}") if bad else ("pass", f"{len(found)} connects, all loopback, no DNS")


def js_result(log: EventLog, mode: str) -> tuple[str, str]:
    on, off = report_value(log, mode, "js") == "on", report_value(log, mode, "js") == "off"
    want_off = MODES[mode]["PROBE_JS"] == "off"
    ok = (off and not on) if want_off else on
    return ("pass" if ok else "FAIL"), f"js={'on' if on else 'off' if off else 'no report'}"


def evaluate(log: EventLog, mode: str) -> list[tuple[str, str, str]]:
    m = mode
    rows = [
        ("page loaded via proxy", *page_result(log, m)),
        ("JavaScript state", *js_result(log, m)),
        ("http image via proxy", *(("pass", "") if via_proxy(log, m, f"img-http-{m}") else ("info", "not seen"))),
        ("https image via proxy", *(("pass", "") if via_proxy(log, m, f"img-https-{m}") else ("info", "not seen"))),
        ("clearnet IP fetch via proxy", *(("pass", "") if via_proxy(log, m, "203.0.113.7") else ("info", "not seen"))),
        ("WebSocket via proxy", *(("pass", "") if via_proxy(log, m, f"ws-{m}") else ("info", "not seen"))),
        ("loopback 127.0.0.1", *direct_result(log, m, f"lb-ip-{m}", "loopback")),
        ("loopback localhost", *direct_result(log, m, f"lb-name-{m}", "loopback")),
        ("LAN IP", *direct_result(log, m, f"lan-ip-{m}", "lan")),
        ("WebRTC main frame", *rtc_result(log, m, "")),
        ("WebRTC iframe", *rtc_result(log, m, "frame_")),
        ("UDP to STUN canary", *(("LEAK", "UDP left the engine") if log.of(m, "udp") else ("pass", "no packets"))),
        ("strace connects", *strace_result(m)),
    ]
    if m in TRAP_MODES:
        rows.insert(1, ("trap page on the LAN canary", *direct_result(log, m, f"trap-{m}", "lan")))
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
    sys.stdout.reconfigure(encoding="utf-8")
    args = parse_args()
    log = EventLog()
    ports = {"udp": serve_udp(log).getsockname()[1]}
    ports["canary"] = serve_http(CanaryHandler, log, ports).server_port
    lan = lan_address()
    ports["lan"] = f"{lan}:{serve_http(CanaryHandler, log, ports, lan).server_port}"
    ports["proxy"] = serve_http(ProxyHandler, log, ports).server_port
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
