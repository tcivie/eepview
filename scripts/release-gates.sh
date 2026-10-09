#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
# Fail unless the tagged commit is on a release branch and every required check passed on it.
# Required checks come from the active ruleset that targets the release branches (refs/heads/release/**):
# the fast lane (gate) and the heavy lane (heavy-gate). See docs/wiki/release-pipeline.md.
# Env: REPO (owner/name), SHA (the tagged commit). Needs a full checkout with the release branches.
# The caller must check out with fetch-depth 0, so the origin/release/* refs exist.
set -euo pipefail

: "${REPO:?set REPO to owner/name}"
: "${SHA:?set SHA to the tagged commit}"

# Print one required check name per line.
required_checks() {
  local id
  for id in $(gh api "repos/${REPO}/rulesets" \
    --jq '.[] | select(.enforcement == "active" and .target == "branch") | .id'); do
    gh api "repos/${REPO}/rulesets/${id}" --jq '
      select(.conditions.ref_name.include | any(startswith("refs/heads/release/")))
      | .rules[] | select(.type == "required_status_checks")
      | .parameters.required_status_checks[].context'
  done | sort -u
}

# Print the SHA of the commit and the head SHA of each PR that merged it.
# Some required checks run on pull requests only, so they exist on the PR head, not on the squash commit.
candidate_shas() {
  echo "$SHA"
  gh api "repos/${REPO}/commits/${SHA}/pulls" --jq '.[].head.sha'
}

# Print the names of check runs that ended in success on the commit or on its PR heads.
passed_checks() {
  local sha
  for sha in $(candidate_shas); do
    gh api "repos/${REPO}/commits/${sha}/check-runs" --paginate \
      --jq '.check_runs[] | select(.conclusion == "success") | .name'
  done | sort -u
}

main() {
  if [ -z "$(git branch -r --contains "$SHA" --list 'origin/release/*')" ]; then
    echo "::error::$SHA is not on a release branch (release/*)"
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
