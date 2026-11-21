#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
#
# Score ratchet: the file-based Scorecard checks may stay the same or go up, never down.
# Floors live in .github/score-floors.json.
#   pr  Run the Scorecard CLI on this checkout for the file-based checks.
# Set REPORT_FILE to also write the drops as a Markdown table.
# Needs Docker and jq.
# Usage: scripts/score-ratchet.sh pr
set -euo pipefail

cd "$(dirname "$0")/.."

FLOORS=.github/score-floors.json
SCORECARD_IMAGE="ghcr.io/ossf/scorecard@sha256:3f24714e9366917adb7a05635382c97dfecb14b21eaef3dfa2ea48c8e23e0795"
PR_CHECKS="Binary-Artifacts,Dangerous-Workflow,Pinned-Dependencies,Token-Permissions,SAST,Security-Policy,License,Fuzzing,Dependency-Update-Tool,Packaging"

pr_floors() {
  jq -c '.scorecard_pr.checks | with_entries(.key |= "scorecard/" + .)' "$FLOORS"
}

# Flat check scores from a Scorecard JSON document on stdin.
scorecard_checks() {
  jq -c '.checks | map({key: ("scorecard/" + .name), value: .score}) | from_entries'
}

pr_now() {
  docker run --rm -v "$PWD:/repo:ro" "$SCORECARD_IMAGE" \
    --local=/repo --checks="$PR_CHECKS" --format=json | scorecard_checks
}

# Lines "name<TAB>floor<TAB>now" for every score below its floor.
find_drops() {
  jq -nr --argjson f "$1" --argjson n "$2" '
    $f | to_entries[]
    | select(($n[.key] // -2) < .value)
    | [.key, .value, ($n[.key] // "missing")] | @tsv'
}

# Lines "name<TAB>floor<TAB>now" for every score above its floor.
find_raises() {
  jq -nr --argjson f "$1" --argjson n "$2" '
    $f | to_entries[]
    | select(($n[.key] // -2) > .value)
    | [.key, .value, $n[.key]] | @tsv'
}

write_report() {
  [ -n "${REPORT_FILE:-}" ] || return 0
  {
    echo "| Score | Floor | Now |"
    echo "| --- | --- | --- |"
    awk -F'\t' '{ printf "| %s | %s | %s |\n", $1, $2, $3 }' <<<"$1"
  } >"$REPORT_FILE"
}

fail_on_drops() {
  local drops="$1"
  [ -n "$drops" ] || return 0
  echo "error: these scores dropped below their floor:" >&2
  awk -F'\t' '{ printf "  %s: floor %s, now %s\n", $1, $2, $3 }' <<<"$drops" >&2
  write_report "$drops"
  return 1
}

# Print the floors file with the pr floors raised to the current scores.
print_raised_floors() {
  jq --argjson n "$1" '
    .scorecard_pr.checks |= with_entries(.value = ([.value, $n["scorecard/" + .key]] | max))' "$FLOORS"
}

print_raise_lines() {
  [ -n "$1" ] || return 0
  awk -F'\t' '{ printf "raise the floor: %s %s -> %s\n", $1, $2, $3 }' <<<"$1"
}

report_raises() {
  local raises="$1" now="$2"
  [ -n "$raises" ] || return 0
  print_raise_lines "$raises"
  echo "Paste this into $FLOORS:"
  print_raised_floors "$now"
}

run_pr() {
  local floors now drops
  floors="$(pr_floors)"
  now="$(pr_now)"
  drops="$(find_drops "$floors" "$now")"
  fail_on_drops "$drops"
  echo "ok: no file-based Scorecard check is below its floor."
  report_raises "$(find_raises "$floors" "$now")" "$now"
}

case "${1:-}" in
  pr) run_pr ;;
  *)
    echo "usage: $0 pr" >&2
    exit 2
    ;;
esac
