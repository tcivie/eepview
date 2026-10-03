#!/usr/bin/env python3
"""eepview leak test. Runs the real eepview binary against a fake I2P proxy and canaries.

Exit code 0 only when every canary got zero hits, the fake upstream saw only `.i2p` hosts,
the test page loaded and reported, and (on Linux, with --strace) the app opened no socket
except to its own gatekeeper and the fake upstream. See docs/adr/0001-no-leak-architecture.md.

  python3 tests/leak/harness.py --binary src-tauri/target/release/eepview
  python3 tests/leak/harness.py --negative-control
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path

from servers import EventLog, start_detectors
from verdict import Run, evaluate, failures

# The default run is the one that counts: JavaScript is on by default in eepview.
# The js-off run is a sanity check. Its leak rows still fail the test.
RUNS = {
    "default": ({}, "on", True),
    "js-off": ({"EEPVIEW_JS": "off"}, "off", False),
}
START_URL = "http://leaktest.i2p/"
PROXY_VARS = ("http_proxy", "https_proxy", "all_proxy", "no_proxy")


def app_env(ports: dict, run: str, exit_after: int) -> dict:
    env = {k: v for k, v in os.environ.items() if k.lower() not in PROXY_VARS}
    env.update(RUNS[run][0])
    env["EEPVIEW_PROXY"] = f"127.0.0.1:{ports['upstream']}"
    env["EEPVIEW_START_URL"] = START_URL
    env["EEPVIEW_EXIT_AFTER"] = str(exit_after)
    return env


def app_command(args, strace_log: Path | None) -> list[str]:
    cmd = [str(Path(args.binary).resolve())]
    if strace_log:
        trace = "trace=connect,sendto,sendmsg,bind,listen,getsockname"
        cmd = ["strace", "-f", "-qq", "-e", trace, "-o", str(strace_log), *cmd]
    if args.xvfb:
        cmd = ["xvfb-run", "-a", *cmd]
    return cmd


def run_app(log: EventLog, ports: dict, name: str, args) -> Run:
    env_extra, expect_js, counts = RUNS[name]
    run = Run(
        name,
        expect_js,
        counts,
        strace=args.out / f"strace.{name}.log" if args.strace else None,
    )
    log.run = name
    with open(args.out / f"app.{name}.log", "wb") as out:
        try:
            done = subprocess.run(
                app_command(args, run.strace),
                env=app_env(ports, name, args.exit_after),
                stdout=out,
                stderr=subprocess.STDOUT,
                timeout=args.exit_after + 60,
                check=False,
            )
            run.code = done.returncode
        except subprocess.TimeoutExpired:
            run.code = None
    time.sleep(2)  # late packets still count against this run
    log.run = "idle"
    return run


def print_table(title: str, rows: list[tuple]) -> None:
    print(
        f"\n### {sys.platform} — {title}\n\n| Check | Result | Detail |\n|---|---|---|"
    )
    for check, result, detail in rows:
        print(f"| {check} | {result} | {str(detail).replace('|', '/')} |")


def leak_test(args) -> int:
    args.out.mkdir(parents=True, exist_ok=True)
    log = EventLog()
    ports = start_detectors(log)
    results = {}
    for name in args.runs.split(","):
        run = run_app(log, ports, name, args)
        results[name] = evaluate(log, ports, run)
        print_table(f"run `{name}`", results[name])
    report = {
        "platform": sys.platform,
        "ports": ports,
        "results": results,
        "events": log.events,
    }
    (args.out / "leak-results.json").write_text(json.dumps(report, indent=2))
    bad = [row for rows in results.values() for row in failures(rows)]
    print(f"\n**{'FAIL' if bad else 'PASS'}** — {len(bad)} failing check(s)")
    return 1 if bad else 0


def parse_args(argv: list[str]):
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("--binary", help="the eepview release binary")
    ap.add_argument(
        "--runs", default=",".join(RUNS), help=f"comma list from {list(RUNS)}"
    )
    ap.add_argument("--exit-after", type=int, default=20)
    ap.add_argument(
        "--xvfb", action="store_true", help="run the app under xvfb-run (Linux CI)"
    )
    ap.add_argument(
        "--strace", action="store_true", help="trace the app's sockets (Linux)"
    )
    ap.add_argument("--out", type=Path, default=Path("leak-results"))
    ap.add_argument(
        "--negative-control", action="store_true", help="prove the detectors can fail"
    )
    args = ap.parse_args(argv)
    if not args.negative_control and not args.binary:
        ap.error("--binary is required")
    return args


def main(argv: list[str]) -> int:
    sys.stdout.reconfigure(encoding="utf-8")
    args = parse_args(argv)
    if args.negative_control:
        from control import negative_control

        return negative_control(args.out)
    return leak_test(args)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
