# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT

"""Detectors for the leak test: a fake upstream I2P proxy and the canaries.

The fake upstream acts like an I2P HTTP proxy with no outproxy. It never connects anywhere.
It logs the host of every request it gets. The canaries log every connection or datagram that
reaches them. A page in eepview must never reach a canary.
"""

from __future__ import annotations

import errno
import socket
import socketserver
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlsplit

PAGES_DIR = Path(__file__).resolve().parent / "pages"
SITE_HOSTS = ("leaktest.i2p", "frame.leaktest.i2p")
VERIFY_HOST = "proxy.i2p"
CLEARNET_REDIRECT = "http://redirect.clear.example/r"
CONTENT_TYPES = {
    ".html": "text/html; charset=utf-8",
    ".js": "text/javascript; charset=utf-8",
}


class EventLog:
    """A thread-safe list of events. Each event carries the name of the run that was active."""

    def __init__(self) -> None:
        self.events: list[dict] = []
        self.run = "idle"
        self.canaries: list[str] = []
        self.lock = threading.Lock()

    def add(self, kind: str, **fields: str) -> None:
        with self.lock:
            self.events.append({"kind": kind, "run": self.run, **fields})

    def select(self, kind: str, run: str, strict: bool = False) -> list[dict]:
        """Events of `kind` in `run`. Unless `strict`, events outside every run count too."""
        runs = {run} if strict else {run, "idle"}
        with self.lock:
            return [e for e in self.events if e["kind"] == kind and e["run"] in runs]


def normalize_host(raw: str) -> str:
    """Lowercase, no userinfo, no port, no trailing dot. An IPv6 literal keeps its brackets."""
    host = raw.strip().lower().rsplit("@", 1)[-1]
    if host.startswith("["):
        return host.split("]", 1)[0] + "]"
    return host.split(":", 1)[0].rstrip(".")


def is_i2p(host: str) -> bool:
    return len(host) > len(".i2p") and host.endswith(".i2p")


def render(name: str, ports: dict) -> bytes:
    text = (PAGES_DIR / name).read_text(encoding="utf-8")
    if name.endswith(".html"):
        for key in ("canary", "lan_tcp", "udp", "lan_udp", "frame_wait"):
            text = text.replace(f"__{key.upper()}__", str(ports.get(key, "")))
    return text.encode()


class LeakHTTPServer(ThreadingHTTPServer):
    """The fake upstream. It carries the event log and the ports the pages point at."""

    daemon_threads = True
    # The page fires a burst of parallel requests, and the gatekeeper opens one upstream
    # connection for each. The socketserver default backlog is 5. On macOS a full listen
    # queue drops the SYN, the client retries after about 1 s, and the gatekeeper gives up
    # its UPSTREAM_CONNECT wait after 500 ms: it answers 502 and the request is lost. A real
    # I2P router proxy has a large backlog, so the fake one must have one too.
    request_queue_size = 128

    def __init__(self, log: EventLog, ports: dict) -> None:
        super().__init__(("127.0.0.1", 0), UpstreamHandler)
        self.log = log
        self.ports = ports

    def handle_error(self, request, client_address) -> None:
        self.log.add("upstream_error", detail=f"connection from {client_address} broke")


class UpstreamHandler(BaseHTTPRequestHandler):
    """Serves VERIFY and the test site, refuses clearnet with 503, logs every host."""

    server: LeakHTTPServer

    def log_message(self, *_args) -> None:
        return

    def log_error(self, *args) -> None:
        self.server.log.add(
            "upstream_error", detail=args[0] % args[1:], method=str(self.command)
        )

    def reply(self, status: int, body: bytes, headers: dict | None = None) -> None:
        self.send_response(status)
        for key, value in {"Content-Type": "text/html", **(headers or {})}.items():
            self.send_header(key, value)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def do_CONNECT(self) -> None:
        host, _, port = self.path.rpartition(":")
        host = normalize_host(host)
        self.server.log.add("upstream", method="CONNECT", host=host, path=self.path)
        if not is_i2p(host):
            return self.reply(503, b"No Outproxy Configured")
        if port != "80":
            return self.reply(502, b"the fake upstream has no TLS")
        # An I2P proxy tunnels CONNECT to the eepsite. WKWebView sends plain http as CONNECT.
        self.send_response(200, "Connection established")
        self.end_headers()
        self.wfile.flush()
        UpstreamHandler(self.connection, self.client_address, self.server)
        self.close_connection = True
        return None

    def do_GET(self) -> None:
        url = urlsplit(self.path)
        host = normalize_host(url.netloc or self.headers.get("Host", ""))
        self.server.log.add("upstream", method=self.command, host=host, path=url.path)
        if not is_i2p(host):
            return self.reply(503, b"<h1>No Outproxy Configured</h1>")
        if host == VERIFY_HOST and url.path == "/":
            return self.reply(200, b"I2P HTTP proxy OK", {"Content-Type": "text/plain"})
        if host in SITE_HOSTS:
            return self.serve_site(host, url)
        return self.reply(404, b"unknown eepsite")

    do_POST = do_GET
    do_HEAD = do_GET
    do_OPTIONS = do_GET
    do_PUT = do_GET
    do_DELETE = do_GET

    def serve_site(self, host: str, url) -> None:
        if url.path == "/report":
            query = {k: v[0] for k, v in parse_qs(url.query).items()}
            self.server.log.add(
                "report", host=host, k=query.get("k", ""), v=query.get("v", "")
            )
            return self.reply(204, b"")
        if url.path == "/redirect":
            return self.reply(302, b"", {"Location": CLEARNET_REDIRECT})
        name = "page.html" if url.path == "/" else url.path.lstrip("/")
        if name not in site_files():
            return self.reply(404, b"not found")
        content_type = CONTENT_TYPES[Path(name).suffix]
        return self.reply(
            200, render(name, self.server.ports), {"Content-Type": content_type}
        )


def site_files() -> set[str]:
    return {p.name for p in PAGES_DIR.iterdir() if p.suffix in CONTENT_TYPES}


class CanaryServer(socketserver.ThreadingTCPServer):
    daemon_threads = True

    def __init__(self, address: tuple, name: str, log: EventLog) -> None:
        self.address_family = socket.AF_INET6 if ":" in address[0] else socket.AF_INET
        super().__init__(address, CanaryHandler)
        self.name = name
        self.log = log

    def handle_error(self, request, client_address) -> None:
        return  # the hit is already logged; a reset by the client changes nothing


class CanaryHandler(socketserver.BaseRequestHandler):
    """Logs every accepted connection, even one that never sends a byte (a preconnect)."""

    server: CanaryServer

    def handle(self) -> None:
        self.request.settimeout(2)
        try:
            first = self.request.recv(512).split(b"\r\n", 1)[0]
        except OSError:
            first = b""
        peer = f"{self.client_address[0]}:{self.client_address[1]}"
        self.server.log.add(
            "canary", canary=self.server.name, peer=peer, first=first.decode("latin-1")
        )
        try:
            self.request.sendall(
                b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\ncanary"
            )
        except OSError:
            return


def serve(server: socketserver.BaseServer) -> None:
    threading.Thread(target=server.serve_forever, daemon=True).start()


def start_loopback_canary(log: EventLog) -> int:
    """One TCP port on both 127.0.0.1 and ::1, so `localhost` hits it whichever it resolves to."""
    for _ in range(20):
        v4 = CanaryServer(("127.0.0.1", 0), "tcp 127.0.0.1", log)
        port = v4.server_address[1]
        try:
            servers = [v4, CanaryServer(("::1", port), "tcp [::1]", log)]
        except OSError as err:
            if err.errno == errno.EADDRINUSE:
                v4.server_close()
                continue
            servers = [
                v4
            ]  # This host has no IPv6 loopback, so `localhost` can only be 127.0.0.1.
        for server in servers:
            serve(server)
            log.canaries.append(server.name)
        return port
    raise RuntimeError("no free port on both 127.0.0.1 and ::1")


def start_udp_canary(log: EventLog, host: str) -> int:
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind((host, 0))
    name = f"udp {host}"
    log.canaries.append(name)

    def loop() -> None:
        while True:
            _data, addr = sock.recvfrom(4096)
            log.add(
                "canary", canary=name, peer=f"{addr[0]}:{addr[1]}", first="datagram"
            )

    threading.Thread(target=loop, daemon=True).start()
    return sock.getsockname()[1]


def default_route_address() -> list[str]:
    """The address of the default route. A UDP connect sends no packet."""
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
            sock.connect(("192.0.2.1", 9))
            return [sock.getsockname()[0]]
    except OSError:
        return []


def host_addresses() -> list[str]:
    try:
        infos = socket.getaddrinfo(socket.gethostname(), None, socket.AF_INET)
    except OSError:
        return []
    return [info[4][0] for info in infos]


def reachable(address: str) -> bool:
    """A VPN tunnel address can block traffic to itself. Then the canary on it proves nothing."""
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as server:
        server.bind((address, 0))
        server.listen(1)
        try:
            socket.create_connection(server.getsockname(), timeout=1).close()
        except OSError:
            return False
    return True


def lan_address() -> str | None:
    """A non-loopback address of this host that a local connection can reach."""
    for address in default_route_address() + host_addresses():
        if not address.startswith(("127.", "0.")) and reachable(address):
            return address
    return None


def start_lan_canaries(log: EventLog, ports: dict) -> None:
    lan = lan_address()
    if lan is None:
        return
    tcp = CanaryServer((lan, 0), f"tcp {lan}", log)
    serve(tcp)
    log.canaries.append(tcp.name)
    ports["lan"] = lan
    ports["lan_tcp"] = f"{lan}:{tcp.server_address[1]}"
    ports["lan_udp"] = f"{lan}:{start_udp_canary(log, lan)}"


def start_detectors(log: EventLog) -> dict:
    """Start every canary and the fake upstream. Returns the ports the pages and the app use."""
    ports: dict = {
        "canary": start_loopback_canary(log),
        "udp": start_udp_canary(log, "127.0.0.1"),
    }
    start_lan_canaries(log, ports)
    upstream = LeakHTTPServer(log, ports)
    serve(upstream)
    ports["upstream"] = upstream.server_address[1]
    return ports
