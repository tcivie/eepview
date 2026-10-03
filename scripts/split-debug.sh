#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
# Move the debug symbols of the release binary into a separate file, for upload as a release asset.
# - Linux: objcopy copies the DWARF into eepview-<target>.debug, then strips the binary and adds a debug link.
# - macOS: cargo writes eepview.dSYM (split-debuginfo = "packed"); this zips it.
# - Windows: cargo writes eepview.pdb next to the exe; this copies it.
# Usage: scripts/split-debug.sh <target> <out-dir>
set -euo pipefail

target="${1:?usage: split-debug.sh <target> <out-dir>}"
out="${2:?usage: split-debug.sh <target> <out-dir>}"
release="src-tauri/target/$target/release"
mkdir -p "$out"

case "$target" in
  *-linux-*)
    objcopy --only-keep-debug "$release/eepview" "$out/eepview-$target.debug"
    objcopy --strip-all --add-gnu-debuglink="$out/eepview-$target.debug" "$release/eepview"
    ;;
  *-apple-*)
    # cargo uplifts eepview.dSYM as a relative symlink into deps/. Resolve it, so the zip holds the DWARF bundle.
    dsym="$(cd "$release/eepview.dSYM" && pwd -P)"
    ditto -c -k --keepParent "$dsym" "$out/eepview-$target.dSYM.zip"
    unzip -l "$out/eepview-$target.dSYM.zip" | grep "Contents/Resources/DWARF/eepview" >/dev/null
    ;;
  *-windows-*)
    cp "$release/eepview.pdb" "$out/eepview-$target.pdb"
    ;;
  *)
    echo "error: no debug-symbol rule for $target" >&2
    exit 1
    ;;
esac

ls -l "$out"
