#!/usr/bin/env bash
# Keep docs on track.
# 1. A PR that changes code, dependencies, permissions, scripts or workflows must also change CHANGELOG.md or docs/wiki.
# 2. Every docs/wiki page except README.md must be linked from docs/wiki/README.md.
# 3. Docs live in docs/wiki/. A tracked .md, .mdx or .rst file, or a root or docs/ .txt file, outside the allowlist fails the check.
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

ALLOWED_DOCS=(
  '^README\.md$'
  '^LICENSE([-.][^/]*)?$'
  '^SECURITY\.md$'
  '^CONTRIBUTING\.md$'
  '^CODE_OF_CONDUCT\.md$'
  '^CHANGELOG\.md$'
  '^AGENTS\.md$'
  '^CLAUDE\.md$'
  '^\.github/'
  '^docs/wiki/'
  '(^|/)requirements[^/]*\.txt$'
)

check_placement() {
  local status=0 file pattern allowed
  while IFS= read -r file; do
    allowed=0
    for pattern in "${ALLOWED_DOCS[@]}"; do
      if [[ "$file" =~ $pattern ]]; then
        allowed=1
        break
      fi
    done
    if [ "$allowed" -eq 0 ]; then
      echo "error: $file"
      echo "Docs live in docs/wiki/. Move this file there." >&2
      status=1
    fi
  done < <(git ls-files '*.md' '*.mdx' '*.rst' ':(glob)*.txt' ':(glob)docs/**/*.txt')
  return "$status"
}

: "${BASE_REF:?BASE_REF must be set}"
check_changes
check_index
check_placement
