#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
# Keep docs on track.
# 1. A PR whose title starts with feat (feat:, feat(scope):, feat!:) must change a page under docs/wiki/. Other types pass without a wiki change.
# 2. A PR must not edit CHANGELOG.md. The release job generates it. The one exception is the generated file itself:
#    a changed CHANGELOG.md passes only when it equals the output of scripts/changelog.sh for the base branch,
#    or for the commit where the PR branched off it.
# 3. Every docs/wiki page except README.md must be linked from docs/wiki/README.md.
# 4. Docs live in docs/wiki/. A tracked .md, .mdx or .rst file, or a root or docs/ .txt file, outside the allowlist fails the check.
# The PR title comes from PR_TITLE, or from the event payload at GITHUB_EVENT_PATH in CI.
# Usage: BASE_REF=main PR_TITLE='feat: x' scripts/docs-check.sh
set -euo pipefail

cd "$(dirname "$0")/.."

FEAT_TITLE='^feat(\([^)]*\))?!?:'

pr_title() {
  if [ -n "${PR_TITLE:-}" ]; then
    printf '%s' "$PR_TITLE"
    return 0
  fi
  if [ -z "${GITHUB_EVENT_PATH:-}" ]; then
    echo "error: set PR_TITLE, or run in CI where GITHUB_EVENT_PATH is set." >&2
    return 1
  fi
  jq -r '.pull_request.title // empty' "$GITHUB_EVENT_PATH"
}

# The tip of the base branch, and the commit where the PR branched off it. A refresh made against either one passes,
# so a merge to main after the branch started does not turn a correct refresh red.
changelog_matches_generated() {
  local base fork rev head
  base="origin/${BASE_REF}"
  head="HEAD"
  if git rev-parse --verify -q 'HEAD^2' >/dev/null; then
    head="HEAD^2"
  fi
  fork="$(git merge-base "$base" "$head")"
  for rev in "$base" "$fork"; do
    if ./scripts/changelog.sh "$rev" | cmp -s - CHANGELOG.md; then
      return 0
    fi
  done
  return 1
}

check_feat_docs() {
  if [[ ! "$TITLE" =~ $FEAT_TITLE ]]; then
    return 0
  fi
  if grep -q '^docs/wiki/' <<<"$CHANGED"; then
    return 0
  fi
  echo "error: the PR title starts with feat, but no page under docs/wiki/ changed. Update or add the wiki page." >&2
  return 1
}

check_changelog() {
  if ! grep -qx 'CHANGELOG\.md' <<<"$CHANGED"; then
    return 0
  fi
  if changelog_matches_generated; then
    return 0
  fi
  echo "error: CHANGELOG.md changed by hand. git-cliff generates it from the commit titles. Revert the change and write a clear Conventional Commit PR title." >&2
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
# Read the inputs once, here, so a failure stops the script (set -e is off inside a function that runs in a || list).
TITLE="$(pr_title)"
CHANGED="$(git diff --name-only "origin/${BASE_REF}...HEAD")"

status=0
check_feat_docs || status=1
check_changelog || status=1
check_index || status=1
check_placement || status=1
exit "$status"
