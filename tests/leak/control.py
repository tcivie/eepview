# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT

"""Negative control: prove that the leak test CAN fail.

The harness itself leaks on purpose, with no eepview involved: it connects straight to every
canary, asks the fake upstream for a clearnet host, loads no page, and hands the strace parser a
log with a clearnet connect and a DNS query. Every detector must report LEAK or FAIL.
Exit code 0 means every detector fired.
"""

from __future__ import annotations

import socket
import time
import urllib.error
import urllib.request
from pathlib import Path

from servers import EventLog, start_detectors
from verdict import Run, evaluate

SYNTHETIC_STRACE = """\
4242 connect(31, {sa_family=AF_INET, sin_port=htons(443), sin_addr=inet_addr("93.184.216.34")}, 16) = 0
4243 sendto(32, "\\x12\\x34", 2, 0, {sa_family=AF_INET, sin_port=htons(53), sin_addr=inet_addr("127.0.0.53")}, 16) = 2
"""


def fetch(url: str, proxy: str | None) -> None:
    opener = urllib.request.build_opener(
        urllib.request.ProxyHandler({"http": proxy} if proxy else {})
    )
    try:
        opener.open(url, timeout=5).read()
    except urllib.error.HTTPError:
        return  # 503 from the fake upstream is expected


def leak_on_purpose(ports: dict) -> None:
    upstream = f"http://127.0.0.1:{ports['upstream']}"
    leaks = [
        lambda: fetch(f"http://127.0.0.1:{ports['canary']}/control", None),
        lambda: fetch(f"http://[::1]:{ports['canary']}/control", None),
        lambda: fetch("http://clearnet.example/control", upstream),
        lambda: udp_send(f"127.0.0.1:{ports['udp']}"),
    ]
    if "lan" in ports:
        leaks.append(lambda: fetch(f"http://{ports['lan_tcp']}/control", None))
        leaks.append(lambda: udp_send(ports["lan_udp"]))
    for leak in leaks:
        try:
            leak()
        except OSError as err:
            print(f"a deliberate leak could not be sent: {err}")


def udp_send(address: str) -> None:
    host, port = address.rsplit(":", 1)
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
        sock.sendto(b"\x00\x01\x00\x00control", (host, int(port)))


def missed(rows: list[tuple]) -> list[tuple]:
    """Rows that must have fired but did not."""
    must_fire = (
        "canary ",
        "upstream saw only",
        "page served",
        "page rendered",
        "strace",
    )
    return [
        r for r in rows if r[0].startswith(must_fire) and r[1] not in ("LEAK", "FAIL")
    ]


def negative_control(out: Path) -> int:
    out.mkdir(parents=True, exist_ok=True)
    log = EventLog()
    ports = start_detectors(log)
    log.run = "control"
    leak_on_purpose(ports)
    time.sleep(1)
    trace = out / "strace.control.log"
    trace.write_text(SYNTHETIC_STRACE)
    rows = evaluate(log, ports, Run("control", "on", True, code=0, strace=trace))
    print("\n### negative control\n\n| Check | Result | Detail |\n|---|---|---|")
    for check, result, detail in rows:
        print(f"| {check} | {result} | {detail} |")
    bad = missed(rows)
    print(
        f"\n**{'FAIL: a detector stayed silent' if bad else 'PASS: every detector fired'}** {bad}"
    )
    return 1 if bad else 0
