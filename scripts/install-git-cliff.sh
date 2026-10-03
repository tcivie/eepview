#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
# Install the pinned git-cliff into a directory, after a SHA-256 check, and add that directory to PATH for later CI steps.
# Usage: scripts/install-git-cliff.sh <directory>
# Raise the version and the checksum together. Take the checksum from the release's .sha512 file or compute it yourself.
set -euo pipefail

VERSION="2.14.2"
SHA256="24f397c733add5390fdceee3a2088588ab0d5f944ce00d34cb7029b888cf2db4"
ARCHIVE="git-cliff-${VERSION}-x86_64-unknown-linux-gnu.tar.gz"

dest="${1:?usage: install-git-cliff.sh <directory>}"
mkdir -p "$dest"

curl -fsSL -o "$dest/$ARCHIVE" "https://github.com/orhun/git-cliff/releases/download/v${VERSION}/${ARCHIVE}"
echo "${SHA256}  $dest/$ARCHIVE" | sha256sum -c -
tar -xzf "$dest/$ARCHIVE" -C "$dest" --strip-components=1 "git-cliff-${VERSION}/git-cliff"
rm -f "$dest/$ARCHIVE"

if [ -n "${GITHUB_PATH:-}" ]; then
  echo "$dest" >> "$GITHUB_PATH"
fi
"$dest/git-cliff" --version
