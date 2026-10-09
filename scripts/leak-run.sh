#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
# Builds the app in the form that users run on this OS, then runs the leak harness against it.
# CI calls it from the leak jobs: ci.yml (debug build, Linux) and heavy.yml (release build, every OS).
# Usage: scripts/leak-run.sh release|debug
# Env: RUNNER_OS (Linux, macOS or Windows). GITHUB_STEP_SUMMARY, when set, gets the harness verdict.
# See docs/wiki/leak-test.md.
set -euo pipefail

profile="${1:-}"
case "$profile" in
  release)
    dir=release
    build=()
    ;;
  debug)
    dir=debug
    build=(--debug)
    ;;
  *)
    echo "usage: $0 release|debug" >&2
    exit 2
    ;;
esac

# macOS runs the .app: its Info.plist turns on App Transport Security, which a bare binary never
# meets (the tabs-never-load bug of #85). Linux and Windows run the binary.
harness=()
case "${RUNNER_OS:-}" in
  Linux)
    build+=(--no-bundle)
    binary="src-tauri/target/${dir}/eepview"
    harness=(--xvfb --strace)
    ;;
  macOS)
    build+=(--bundles app)
    binary="src-tauri/target/${dir}/bundle/macos/eepview.app/Contents/MacOS/eepview"
    ;;
  Windows)
    build+=(--no-bundle)
    binary="src-tauri/target/${dir}/eepview.exe"
    ;;
  *)
    echo "set RUNNER_OS to Linux, macOS or Windows" >&2
    exit 2
    ;;
esac

npm run tauri -- build "${build[@]}" -- --locked

if [ "$RUNNER_OS" = Linux ] && [ "$profile" = release ]; then
  ./scripts/check-hardening.sh "$binary"
fi

py="$(command -v python3 || command -v python)"
"$py" tests/leak/harness.py --binary "$binary" --out leak-results ${harness[@]+"${harness[@]}"} |
  tee -a "${GITHUB_STEP_SUMMARY:-/dev/null}"
