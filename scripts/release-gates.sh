#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
# Fail unless the tagged commit is on main and every required check passed on it.
# Required checks come from the active ruleset that targets the default branch.
# Env: REPO (owner/name), SHA (the tagged commit). Needs a full checkout and origin/main.
# The caller must check out with fetch-depth 0, so origin/main exists.
set -euo pipefail

: "${REPO:?set REPO to owner/name}"
: "${SHA:?set SHA to the tagged commit}"

# Print one required check name per line.
required_checks() {
  local id
  for id in $(gh api "repos/${REPO}/rulesets" \
    --jq '.[] | select(.enforcement == "active" and .target == "branch") | .id'); do
    gh api "repos/${REPO}/rulesets/${id}" --jq '
      select(.conditions.ref_name.include | index("~DEFAULT_BRANCH"))
      | .rules[] | select(.type == "required_status_checks")
      | .parameters.required_status_checks[].context'
  done | sort -u
}

# Print the names of check runs that ended in success on the commit.
passed_checks() {
  gh api "repos/${REPO}/commits/${SHA}/check-runs" --paginate \
    --jq '.check_runs[] | select(.conclusion == "success") | .name' | sort -u
}

main() {
  if ! git merge-base --is-ancestor "$SHA" origin/main; then
    echo "::error::$SHA is not on main"
    exit 1
  fi

  local required passed missing
  required="$(required_checks)"
  if [ -z "$required" ]; then
    echo "::error::no required checks found in the ruleset"
    exit 1
  fi
  passed="$(passed_checks)"
  missing="$(comm -23 <(printf '%s\n' "$required") <(printf '%s\n' "$passed"))"
  if [ -n "$missing" ]; then
    echo "::error::required checks missing or not successful on $SHA:"
    printf '%s\n' "$missing"
    exit 1
  fi
  echo "All required checks passed on $SHA:"
  printf '%s\n' "$required"
}

main "$@"
