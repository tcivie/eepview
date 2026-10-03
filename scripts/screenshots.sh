#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
set -euo pipefail

cd "$(dirname "$0")/.."

PORT="${PREVIEW_PORT:-4180}"
BASE="http://127.0.0.1:${PORT}"
PAGE="$BASE/src/ui/index.html"
TRIES=50

fail() {
  echo "error: $*" >&2
  exit 1
}

wait_ready() {
  local _
  for _ in $(seq 1 "$TRIES"); do
    kill -0 "$preview" 2>/dev/null || fail "vite preview exited. Is port ${PORT} free?"
    curl -fs -o /dev/null "$PAGE" && return 0
    sleep 0.2
  done
  fail "vite preview did not answer on port ${PORT}"
}

if curl -fs -o /dev/null "$PAGE"; then
  fail "port ${PORT} already serves a page. Stop that server or set PREVIEW_PORT."
fi

npm run build >/dev/null
npx vite preview --host 127.0.0.1 --port "$PORT" --strictPort >/dev/null 2>&1 &
preview=$!
trap 'kill "$preview" 2>/dev/null || true' EXIT
wait_ready

SCREENSHOT_BASE="$BASE" node scripts/screenshots.mjs
