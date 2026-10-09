#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
# Prints the parts of the repo that a change touches, one "<area>=true|false" line per area.
# The fast lane (.github/workflows/ci.yml) runs a job only when its area is true.
# A change to the fast lane itself marks every area, so the new pipeline tests itself in full.
# Usage: scripts/ci-changes.sh <base> <head>   the change from the merge base of <base> and <head> to <head>
#        scripts/ci-changes.sh --all           every area (a push, or a manual run)
# See docs/wiki/ci-and-quality-gates.md for the jobs of each area.
set -euo pipefail

AREAS=(rust_src rust web macos windows deps workflows leak)

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

print_areas() {
  local area
  for area in "${AREAS[@]}"; do
    case "$hit" in
      *" $area "*) echo "${area}=true" ;;
      *) echo "${area}=false" ;;
    esac
  done
}

# The files that the leak test guards: the no-leak layers L1 to L5
# (docs/wiki/adr-0001-no-leak-architecture.md), the engine settings and the harness itself.
is_leak_path() {
  case "$1" in
    src-tauri/src/net/gatekeeper* | src-tauri/src/net/rules.rs | src-tauri/src/net/host.rs) return 0 ;;
    src-tauri/src/net/loopback.rs | src-tauri/src/nav.rs) return 0 ;;
    src-tauri/src/shell/content.rs | src-tauri/src/shell/engine.rs | src-tauri/src/shell/webrtc.rs) return 0 ;;
    src-tauri/crates/eepview-platform/* | src-tauri/capabilities/*) return 0 ;;
    src-tauri/tauri.conf.json | src-tauri/Info.plist) return 0 ;;
    tests/leak/* | scripts/leak-run.sh) return 0 ;;
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
  case "$path" in
    src-tauri/* | rust-toolchain.toml) mark rust_src rust ;;
  esac
  # The Rust tests read the UI files (theme.css through include_str!, the UI wiring checks),
  # and the app embeds the built UI.
  case "$path" in
    src/* | index.html | package.json | package-lock.json | vite.config.ts | tsconfig*.json) mark rust web ;;
    scripts/check-tested.sh) mark web ;;
  esac
  # A manifest, the lock file, the build script or the app config can break the build on any OS.
  case "$path" in
    src-tauri/Cargo.toml | src-tauri/Cargo.lock | src-tauri/*/Cargo.toml | src-tauri/build.rs) mark macos windows leak ;;
    rust-toolchain.toml | src-tauri/tauri.conf.json) mark macos windows leak ;;
  esac
  case "$path" in
    src-tauri/Cargo.toml | src-tauri/Cargo.lock | src-tauri/*/Cargo.toml | deny.toml) mark deps ;;
    package.json | package-lock.json) mark deps ;;
  esac
  case "$path" in
    src-tauri/*macos* | src-tauri/Info.plist) mark macos ;;
  esac
  case "$path" in
    src-tauri/*windows*) mark windows ;;
  esac
  if is_leak_path "$path"; then
    mark leak
  fi
}

# A Rust file that holds code for one OS only. The Linux jobs never compile that code, so the
# job of that OS must run. The file is read at both ends of the change, so a removed block counts too.
mark_os_specific() {
  local rev
  [ "$#" -gt 2 ] || return 0
  for rev in "$1" "$2"; do
    if git grep -q -E '"macos"|"apple"|macos::' "$rev" -- "${@:3}"; then
      mark macos
    fi
    if git grep -q -E '"windows"|cfg\(windows|cfg\(not\(windows|windows::' "$rev" -- "${@:3}"; then
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
