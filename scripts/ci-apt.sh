#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
# Installs the WebKitGTK build packages on a Linux CI runner in the background, so the install
# runs at the same time as the toolchain, cache and npm steps of the job.
# Usage: scripts/ci-apt.sh start [more packages]   start the install and return at once
#        scripts/ci-apt.sh wait                    wait for the install, print its log, exit with its status
set -euo pipefail

: "${RUNNER_TEMP:?run this on a CI runner}"
log="$RUNNER_TEMP/apt.log"
status="$RUNNER_TEMP/apt.status"
packages=(libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev)

install() {
  sudo apt-get update -q
  sudo apt-get install -y -q --no-install-recommends "${packages[@]}" "$@"
}

start() {
  rm -f "$status"
  # Every stream of the background shell goes to a file: an open stdout keeps the step running.
  (
    if install "$@" >"$log" 2>&1; then code=0; else code=$?; fi
    echo "$code" >"$status.tmp"
    mv "$status.tmp" "$status"
  ) </dev/null >/dev/null 2>&1 &
}

wait_for_install() {
  local waited=0
  until [ -s "$status" ]; do
    if [ "$waited" -ge 600 ]; then
      echo "::error::the package install did not end within 10 minutes"
      cat "$log"
      exit 1
    fi
    sleep 1
    waited=$((waited + 1))
  done
  cat "$log"
  exit "$(cat "$status")"
}

case "${1:-}" in
  start)
    shift
    start "$@"
    ;;
  wait) wait_for_install ;;
  *)
    echo "usage: $0 start [packages] | wait" >&2
    exit 2
    ;;
esac
