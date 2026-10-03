#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
# Print the release notes for one tag: the commits since the previous v* tag, grouped by git-cliff.
# Usage: scripts/release-notes.sh [tag]
# With no tag, it prints the commits of HEAD since the last v* tag under "Unreleased". A dry run uses that.
# Needs a full clone (fetch-depth 0), so the earlier tags exist, and git-cliff on PATH.
set -euo pipefail

cd "$(dirname "$0")/.."

tag="${1:-}"
ref="$(git rev-parse "${tag:-HEAD}")"
previous="$(git describe --tags --abbrev=0 --match 'v[0-9]*' "${ref}^" 2>/dev/null || true)"

args=(--config cliff.toml --strip header)
if [ -n "$tag" ]; then
  args+=(--tag "$tag")
fi
git-cliff "${args[@]}" "${previous:+$previous..}$ref"
