#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
# Fail when a function is too complex or too long, in Rust and TypeScript.
# Limits match clippy.toml and biome.json:
#   cyclomatic complexity <= 10, length <= 40 lines of code, parameters <= 5.
set -euo pipefail

cd "$(dirname "$0")/.."

# lizard imports pygments only for its Erlang reader. scripts/lizard-stubs holds a
# pygments stub, so pygments is not installed. The stub raises if lizard reads Erlang.
# Set LIZARD_PYTHON to a Python that has lizard installed (default: python3).
PYTHONPATH="scripts/lizard-stubs" "${LIZARD_PYTHON:-python3}" -m lizard \
  --CCN 10 \
  --length 40 \
  --arguments 5 \
  --warnings_only \
  -l rust \
  -l typescript \
  --exclude "*/target/*" \
  --exclude "*/node_modules/*" \
  --exclude "*/dist/*" \
  --exclude "*/gen/*" \
  src src-tauri/src src-tauri/crates src-tauri/fuzz
