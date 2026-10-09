#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
# Prints the parts of the repo that a change touches, one "<area>=true|false" line per area.
# The fast lane (.github/workflows/ci.yml) runs a job only when its area is true.
# A change to the fast lane itself marks every area, so the new pipeline tests itself in full.
# The last line, other_os, is the JSON list of the macOS and Windows runners to test on
# (["none"] when there is none), for the matrix of the job that tests them.
# Usage: scripts/ci-changes.sh <base> <head>   the change from the merge base of <base> and <head> to <head>
#        scripts/ci-changes.sh --all           every area (a push, a pull request into a release branch,
#                                              or a manual run)
# See docs/wiki/ci-and-quality-gates.md for the jobs of each area.
set -euo pipefail

AREAS=(rust_src rust web macos windows deps workflows leak heavy)

# The marked areas, each one with a space on both sides. Plain text, so bash 3.2 on macOS runs it too.
hit=" "

mark() {
  local area
  for area in "$@"; do
    case "$hit" in
      *" $area "*) ;;
      *) hit="${hit}${area} " ;;
    esac
  done
}

marked() {
  case "$hit" in
    *" $1 "*) return 0 ;;
  esac
  return 1
}

print_areas() {
  local area runners=()
  for area in "${AREAS[@]}"; do
    if marked "$area"; then echo "${area}=true"; else echo "${area}=false"; fi
  done
  if marked macos; then runners+=('"macos-15"'); fi
  if marked windows; then runners+=('"windows-2025"'); fi
  if [ "${#runners[@]}" -eq 0 ]; then
    echo 'other_os=["none"]'
  else
    echo "other_os=[$(IFS=,; echo "${runners[*]}")]"
  fi
}

# The files that the leak test guards: the no-leak layers L1 to L5
# (docs/wiki/adr-0001-no-leak-architecture.md), VERIFY, the navigation and new-window rules,
# the engine settings and the harness itself.
is_leak_path() {
  case "$1" in
    src-tauri/src/net/* | src-tauri/src/nav.rs | src-tauri/src/core/page.rs) return 0 ;;
    src-tauri/src/shell/content.rs | src-tauri/src/shell/engine.rs | src-tauri/src/shell/webrtc.rs) return 0 ;;
    src-tauri/src/shell/console.rs | src-tauri/src/shell/chrome.rs | src-tauri/src/shell/input.rs) return 0 ;;
    src-tauri/crates/eepview-platform/* | src-tauri/capabilities/*) return 0 ;;
    src-tauri/tauri.conf.json | src-tauri/Info.plist) return 0 ;;
    tests/leak/* | scripts/leak-run.sh | scripts/leak-harness-check.sh) return 0 ;;
  esac
  return 1
}

# The files that only the heavy lane (heavy.yml) runs. A pull request into main that changes
# one of them runs the heavy lane too, so a broken heavy lane shows before the merge.
is_heavy_path() {
  case "$1" in
    .github/workflows/heavy.yml | scripts/leak-run.sh | scripts/leak-harness-check.sh) return 0 ;;
    scripts/ci-apt.sh | scripts/nightly-toolchain.txt | src-tauri/.config/nextest.toml) return 0 ;;
    tests/leak/*) return 0 ;;
  esac
  return 1
}

classify() {
  local path="$1"
  case "$path" in
    .github/workflows/ci.yml | scripts/ci-changes.sh | scripts/ci-apt.sh) mark "${AREAS[@]}" ;;
  esac
  case "$path" in
    .github/*) mark workflows ;;
  esac
  # The Rust tests read the bug report form (src-tauri/tests/diag_review.rs).
  case "$path" in
    .github/ISSUE_TEMPLATE/*) mark rust ;;
  esac
  case "$path" in
    src-tauri/* | rust-toolchain.toml) mark rust_src rust ;;
  esac
  # The Rust tests read the UI files (theme.css through include_str!, the UI wiring checks),
  # and the app embeds the built UI.
  case "$path" in
    src/* | public/* | index.html | package.json | package-lock.json | vite.config.ts | tsconfig*.json) mark rust web ;;
    scripts/check-tested.sh | tests/leak/pages/*) mark web ;;
  esac
  # A manifest, the lock file, the build script or the app config can break the build on any OS.
  case "$path" in
    src-tauri/Cargo.toml | src-tauri/Cargo.lock | src-tauri/*/Cargo.toml | src-tauri/build.rs) mark macos windows leak ;;
    rust-toolchain.toml | src-tauri/tauri.conf.json) mark macos windows leak ;;
  esac
  case "$path" in
    src-tauri/Cargo.toml | src-tauri/Cargo.lock | src-tauri/*/Cargo.toml | src-tauri/deny.toml) mark deps ;;
    package.json | package-lock.json) mark deps ;;
  esac
  case "$path" in
    src-tauri/*macos* | src-tauri/Info.plist) mark macos ;;
  esac
  case "$path" in
    src-tauri/*windows*) mark windows ;;
  esac
  # macos.rs and windows.rs call the shared files of the platform crate.
  case "$path" in
    src-tauri/crates/eepview-platform/*) mark macos windows ;;
  esac
  if is_leak_path "$path"; then
    mark leak
  fi
  if is_heavy_path "$path"; then
    mark heavy
  fi
}

# A Rust file that holds code for one OS only. The Linux jobs never compile that code, so the
# job of that OS must run. The file is read at both ends of the change, so a removed block counts too.
# -w matches whole words: cfg(windows), "windows", windows::, any(windows, ...), "macos", "apple".
# Code under not(unix) is Windows code.
mark_os_specific() {
  local rev
  [ "$#" -gt 2 ] || return 0
  for rev in "$1" "$2"; do
    if git grep -q -w -E 'macos|apple' "$rev" -- "${@:3}"; then
      mark macos
    fi
    if git grep -q -w -e windows "$rev" -- "${@:3}" || git grep -q -F 'not(unix' "$rev" -- "${@:3}"; then
      mark windows
    fi
  done
}

main() {
  if [ "${1:-}" = "--all" ]; then
    mark "${AREAS[@]}"
    print_areas
    return
  fi
  if [ "$#" -ne 2 ]; then
    echo "usage: $0 <base> <head> | --all" >&2
    exit 2
  fi
  local base head path rust_files=()
  base="$(git merge-base "$1" "$2")"
  head="$2"
  while IFS= read -r -d '' path; do
    classify "$path"
    case "$path" in
      src-tauri/*.rs) rust_files+=("$path") ;;
    esac
  done < <(git diff -z --name-only --no-renames "$base" "$head")
  # The ${a[@]+...} form keeps bash 3.2 quiet about an empty array under set -u.
  mark_os_specific "$base" "$head" ${rust_files[@]+"${rust_files[@]}"}
  print_areas
}

main "$@"
