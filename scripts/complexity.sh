#!/usr/bin/env bash
# Fail when a function is too complex or too long, in Rust, TypeScript and Python.
# Limits match clippy.toml and biome.json:
#   cyclomatic complexity <= 10, length <= 40 lines of code, parameters <= 5.
set -euo pipefail

cd "$(dirname "$0")/.."

lizard \
  --CCN 10 \
  --length 40 \
  --arguments 5 \
  --warnings_only \
  -l rust \
  -l typescript \
  -l python \
  --exclude "*/target/*" \
  --exclude "*/node_modules/*" \
  --exclude "*/dist/*" \
  --exclude "*/gen/*" \
  src src-tauri/src spike
