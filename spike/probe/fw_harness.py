#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import os
import secrets
import socket
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

import harness as h

HERE = Path(__file__).resolve().parent
FIREWALL = HERE / "s10" / "firewall.ps1"
FW_PAGE = (HERE / "s10" / "fw_page.html").read_text()

PHASES = {
    "open": ("bare", False),
    "l6": ("bare", True),
    "l6-default": ("default", True),
    "l6-hardened": ("hardened", True),
}
BARE = {"PROBE_HARDEN": "0", "PROBE_JS": "on", "PROBE_WIN_ARGS": "bare"}
OFF_BOX = ("public_ip_http", "public_ip_https", "public_name", "link_local", "azure_host", "websocket")
REACHED = ("resolved", "open")
PROXIED_PAGE = "http://probe.i2p/test.html?mode={phase}"


def ps(action: str, **params: str) -> str:
    cmd = ["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(FIREWALL), "-Action", action]
    for key, value in params.items():
        cmd += [f"-{key}", value]
    return subprocess.run(cmd, capture_output=True, text=True, check=True).stdout.strip()


def ps_json(action: str, **params: str) -> list:
    out = ps(action, **params)
    data = json.loads(out) if out else []
    return data if isinstance(data, list) else [data]


class LoopbackPageHandler(h.ProxyHandler):
    def do_GET(self) -> None:
        url = h.urlsplit(self.path)
        if url.path == "/fw.html":
            mode = h.parse_qs(url.query).get("mode", ["manual"])[0]
            page = FW_PAGE.replace("__TAG__", f"{self.server.ports['nonce']}{mode.replace('-', '')}")
            return self.reply(200, h.render(page, mode, self.server.ports))
        return self.serve_probe(url)


def probe_env(phase: str, ports: dict, seconds: int) -> dict:
    mode = PHASES[phase][0]
    env = {**os.environ, **(BARE if mode == "bare" else h.MODES[mode]), "PROBE_SECONDS": str(seconds)}
    env["PROBE_PROXY"] = f"http://127.0.0.1:{ports['proxy']}"
    env["PROBE_PAGE"] = PROXIED_PAGE.format(phase=phase)
    if mode == "bare":
        env["PROBE_PAGE"] = f"http://127.0.0.1:{ports['page']}/fw.html?mode={phase}"
    return env


def python_exe_without_rule_reaches_internet() -> bool:
    try:
        with socket.create_connection(("1.1.1.1", 80), timeout=5):
            return True
    except OSError:
        return False


def run_phase(phase: str, ports: dict, log: h.EventLog, args) -> dict:
    ps("DnsClear")
    start = datetime.now(timezone.utc).astimezone().isoformat()
    log.mode = phase
    proc = subprocess.Popen([args.binary], env=probe_env(phase, ports, args.seconds))
    time.sleep(args.procs_after)
    procs = ps_json("Procs")
    try:
        proc.wait(timeout=args.seconds + 60)
    except subprocess.TimeoutExpired:
        proc.kill()
        log.add("error", detail="probe did not exit in time")
    time.sleep(args.settle)
    log.mode = "idle"
    return {
        "procs": procs,
        "audit": ps_json("Audit", Since=start),
        "dns_cache": ps_json("DnsDump", Pattern=""),
        "python_internet": python_exe_without_rule_reaches_internet(),
    }


def dns_label_of_phase(phase: str, nonce: str) -> str:
    return f"{nonce}{phase.replace('-', '')}" if PHASES[phase][0] == "bare" else f"-{phase}."


def dns_rows(phase: str, info: dict, pcap: bytes, nonce: str) -> list:
    mark = dns_label_of_phase(phase, nonce)
    probe = mark.strip("-.")
    cached = [e for e in info["dns_cache"] if probe in str(e)]
    on_wire = pcap.count(probe.encode())
    leak = "LEAK" if (cached or on_wire) else "pass"
    wanted = leak if phase != "open" else ("info" if leak == "LEAK" else "warn")
    return [
        ("DNS: names in the system resolver cache", wanted, f"{len(cached)}: {cached[:3]}"),
        ("DNS: names in port-53 packets on the wire", wanted, f"{on_wire} hit(s) for {probe!r}"),
    ]


def runs_from_fixed_runtime(paths: list) -> bool:
    runtime = os.environ.get("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER", "<unset>").lower()
    return bool(paths) and all(p.lower().startswith(runtime) for p in paths)


def process_kind(proc: dict) -> str:
    return f"{proc['type']}:{proc['sub']}" if proc["sub"] else proc["type"]


def engine_flows(info: dict, lan_ip: str) -> tuple[list, list]:
    flows = [f for f in info["audit"] if "msedgewebview2" in f["flow"].lower()]
    allowed = [f for f in flows if f["flow"].startswith("allowed") and f"{lan_ip}:" not in f["flow"]]
    return allowed, [f for f in flows if f not in allowed]


def engine_rows(info: dict, lan_ip: str) -> list:
    paths = sorted({p.get("path") or "?" for p in info["procs"]})
    allowed, other = engine_flows(info, lan_ip)
    online = info["python_internet"]
    return [
        ("engine runs from the fixed runtime", "pass" if runs_from_fixed_runtime(paths) else "FAIL", f"{paths}"),
        ("engine process types", "info", ", ".join(sorted({process_kind(p) for p in info["procs"]}))),
        ("engine flows allowed off-box (WFP 5156)", "LEAK" if allowed else "pass", json.dumps(allowed[:6])),
        ("engine flows blocked or same-host (WFP 5157/5156)", "info", json.dumps(other[:8])),
        ("python.exe still reaches 1.1.1.1:80", "pass" if online else "FAIL", "network is up"),
    ]


def rtc_row(log: h.EventLog, phase: str, must_block: bool) -> tuple[str, str, str]:
    candidates = json.loads(h.report_value(log, phase, "rtc_candidates") or "[]")
    srflx = [c for c in candidates if " typ srflx" in c]
    detail = f"{len(candidates)} candidates, {len(srflx)} srflx (a STUN reply came back)"
    if not must_block:
        return ("WebRTC STUN to public servers", "pass" if srflx else "warn", detail)
    return ("WebRTC STUN to public servers", "LEAK" if srflx else "pass", detail)


def page_rows(log: h.EventLog, phase: str, must_block: bool) -> list:
    rows = [("page script ran (loopback page)", "pass" if h.report_value(log, phase, "js") == "on" else "FAIL", "")]
    for key in OFF_BOX:
        seen = h.report_value(log, phase, key) or "no report"
        reached = seen in REACHED
        verdict = ("LEAK" if reached else "pass") if must_block else ("pass" if reached else "warn")
        rows.append((f"off-box: {key}", verdict, seen))
    rows.append(rtc_row(log, phase, must_block))
    lan = h.report_value(log, phase, "lan_self")
    rows.append(("same-host LAN IP (loopback path, not a real LAN)", "info", f"{lan}; canary hits {len(log.of(phase, 'lan'))}"))
    rows.append(("loopback 127.0.0.1 canary (known limit: L1/L3 own it)", "info", f"hits {len(log.of(phase, 'loopback'))}"))
    return rows


def evaluate(phase: str, log: h.EventLog, info: dict, ctx: dict) -> list:
    mode, ruled = PHASES[phase]
    if mode == "bare":
        rows = page_rows(log, phase, ruled)
    else:
        rows = [("page loaded via the loopback proxy", *h.page_result(log, phase))]
    return rows + engine_rows(info, ctx["lan_ip"]) + dns_rows(phase, info, ctx["pcap"], ctx["nonce"])


def start_servers(log: h.EventLog, nonce: str) -> dict:
    ports = {"udp": h.serve_udp(log).getsockname()[1], "nonce": nonce}
    ports["canary"] = h.serve_http(h.CanaryHandler, log, ports).server_port
    ports["lan_ip"] = h.lan_address()
    ports["lan"] = f"{ports['lan_ip']}:{h.serve_http(h.CanaryHandler, log, ports, ports['lan_ip']).server_port}"
    ports["proxy"] = h.serve_http(h.ProxyHandler, log, ports).server_port
    ports["page"] = h.serve_http(LoopbackPageHandler, log, ports).server_port
    return ports


def run_all(args, log: h.EventLog, ports: dict) -> dict:
    infos = {}
    runtime = os.environ["WEBVIEW2_BROWSER_EXECUTABLE_FOLDER"]
    for phase, (_mode, ruled) in PHASES.items():
        if ruled and not any(PHASES[p][1] for p in infos):
            print(ps("Add", Path=runtime))
        infos[phase] = run_phase(phase, ports, log, args)
    ps("Remove")
    return infos


def gate(results: dict) -> list:
    return [(p, *r) for p, rows in results.items() for r in rows if r[1] in ("LEAK", "FAIL")]


def main() -> int:
    sys.stdout.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser()
    ap.add_argument("--binary", required=True)
    ap.add_argument("--seconds", type=int, default=25)
    ap.add_argument("--out", default="s10-results.json")
    ap.add_argument("--procs-after", type=float, default=10.0)
    ap.add_argument("--settle", type=float, default=2.0)
    args = ap.parse_args()
    log, nonce = h.EventLog(), secrets.token_hex(4)
    ports = start_servers(log, nonce)
    work = Path(os.environ.get("RUNNER_TEMP", HERE))
    ps("CaptureStart", Out=str(work / "dns.etl"))
    infos = run_all(args, log, ports)
    ps("CaptureStop", Path=str(work / "dns.etl"), Out=str(HERE / "s10-dns.pcapng"))
    ctx = {"lan_ip": ports["lan_ip"], "nonce": nonce, "pcap": (HERE / "s10-dns.pcapng").read_bytes()}
    results = {p: evaluate(p, log, infos[p], ctx) for p in PHASES}
    h.print_table(results)
    Path(args.out).write_text(json.dumps({"results": results, "infos": infos, "events": log.events}, indent=2))
    failures = gate(results)
    print(f"\n**Gate:** {'FAIL ' + json.dumps(failures) if failures else 'pass'}")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
