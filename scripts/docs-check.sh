#!/usr/bin/env bash
# Keep docs on track.
# 1. A PR that changes code, dependencies, permissions, scripts or workflows must also change CHANGELOG.md or docs/wiki.
# 2. Every docs/wiki page except README.md must be linked from docs/wiki/README.md.
# Usage: BASE_REF=main scripts/docs-check.sh
set -euo pipefail

cd "$(dirname "$0")/.."

check_changes() {
  local changed
  changed="$(git diff --name-only "origin/${BASE_REF}...HEAD")"
  if ! grep -qE '^(src/|src-tauri/src/|src-tauri/Cargo\.(toml|lock)|src-tauri/tauri\.conf\.json|src-tauri/capabilities/|package\.json|package-lock\.json|scripts/|\.github/workflows/)' <<<"$changed"; then
    return 0
  fi
  if grep -qE '^(CHANGELOG\.md|docs/wiki/)' <<<"$changed"; then
    return 0
  fi
  echo "error: code, dependencies, scripts or workflows changed, but CHANGELOG.md and docs/wiki did not." >&2
  return 1
}

check_index() {
  local status=0 page name
  for page in docs/wiki/*.md; do
    name="$(basename "$page")"
    if [ "$name" = "README.md" ]; then
      continue
    fi
    if ! grep -qF "($name)" docs/wiki/README.md; then
      echo "error: docs/wiki/README.md does not link $name." >&2
      status=1
    fi
  done
  return "$status"
}

: "${BASE_REF:?BASE_REF must be set}"
check_changes
check_index
