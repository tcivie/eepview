#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
#
# Test scripts/score-ratchet.sh with a fake docker and curl. Needs jq.
# Usage: scripts/score-ratchet-test.sh
set -euo pipefail

cd "$(dirname "$0")/.."

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/bin"

# Fake docker prints a Scorecard document with Fuzzing 3 and License 9.
cat >"$tmp/bin/docker" <<'DOCKER'
#!/usr/bin/env bash
cat <<'JSON'
{"checks":[{"name":"Binary-Artifacts","score":10},{"name":"Dangerous-Workflow","score":10},
{"name":"Dependency-Update-Tool","score":10},{"name":"Fuzzing","score":3},{"name":"License","score":9},
{"name":"Packaging","score":-1},{"name":"Pinned-Dependencies","score":10},{"name":"SAST","score":10},
{"name":"Security-Policy","score":10},{"name":"Token-Permissions","score":10}]}
JSON
DOCKER
chmod +x "$tmp/bin/docker"

fail() {
  echo "FAIL: $1" >&2
  exit 1
}

run_pr() {
  PATH="$tmp/bin:$PATH" scripts/score-ratchet.sh pr
}

out="$(run_pr)" || fail "pr mode failed with floors at or below the scores"
grep -q 'raise the floor: scorecard/Fuzzing 0 -> 3' <<<"$out" || fail "no raise line for Fuzzing"
jq -e '.scorecard_pr.checks.Fuzzing == 3 and .scorecard_pr.checks.License == 9 and .scorecard.score == 6.6' \
  <<<"${out#*Paste this into*:}" >/dev/null || fail "pr mode did not print the raised scorecard_pr floors"

cp .github/score-floors.json "$tmp/floors.bak"
trap 'cp "$tmp/floors.bak" .github/score-floors.json; rm -rf "$tmp"' EXIT
jq '.scorecard_pr.checks.License = 10' "$tmp/floors.bak" >.github/score-floors.json
if run_pr >"$tmp/drop.out" 2>&1; then fail "pr mode passed with a score below its floor"; fi
grep -q 'scorecard/License: floor 10, now 9' "$tmp/drop.out" || fail "drop line is missing"

echo "ok: score-ratchet tests passed."
