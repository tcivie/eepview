#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
set -euo pipefail

cd "$(dirname "$0")/.."

PORT="${PREVIEW_PORT:-4180}"
BASE="http://127.0.0.1:${PORT}"

npm run build >/dev/null
npx vite preview --host 127.0.0.1 --port "$PORT" --strictPort >/dev/null 2>&1 &
preview=$!
trap 'kill "$preview" 2>/dev/null || true' EXIT

for _ in $(seq 1 50); do
  curl -fs -o /dev/null "$BASE/src/ui/index.html" && break
  sleep 0.2
done

SCREENSHOT_BASE="$BASE" node scripts/screenshots.mjs
