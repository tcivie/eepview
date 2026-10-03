#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
# Node's coverage report lists only the files that a test loads, so an untested
# module would not count against the gate. Every logic module in src/ui/lib and
# src/ui/shared must therefore have a sibling .test.ts file.
# Usage: scripts/check-tested.sh
set -euo pipefail

cd "$(dirname "$0")/.."

status=0
for file in src/ui/lib/*.ts src/ui/shared/*.ts; do
  case "$file" in
    *.test.ts) continue ;;
  esac
  if [ ! -f "${file%.ts}.test.ts" ]; then
    echo "error: $file has no ${file%.ts}.test.ts, so coverage would not count it." >&2
    status=1
  fi
done
exit "$status"
