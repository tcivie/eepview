#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
# Tests for scripts/ci-changes.sh. Each case builds a small git repo, makes a change on a branch,
# and compares the areas that the script marks true with the expected list.
# Usage: scripts/ci-changes.test.sh
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failures=0
total=0

# A repo with one file of each kind on main. The branch "change" starts at main.
make_repo() {
  rm -rf "$work/repo"
  mkdir -p "$work/repo"
  cd "$work/repo"
  git init -q -b main
  git config user.email test@example.com
  git config user.name test
  mkdir -p docs/wiki src/ui src-tauri/src/core src-tauri/src/net src-tauri/src/shell .github/workflows
  echo page >docs/wiki/page.md
  echo 'export {};' >src/ui/a.ts
  echo 'pub fn a() {}' >src-tauri/src/core/a.rs
  printf '#[cfg(target_os = "macos")]\npub fn m() {}\n' >src-tauri/src/shell/menu.rs
  echo 'pub fn g() {}' >src-tauri/src/net/gatekeeper.rs
  echo lock >src-tauri/Cargo.lock
  echo 'on: push' >.github/workflows/other.yml
  git add -A
  git commit -q -m base
  git checkout -q -b change
}

commit() {
  git add -A
  git commit -q -m change
}

# The areas that the script marks true, sorted, on one line.
areas() {
  "$root/scripts/ci-changes.sh" "$@" | sed -n 's/=true$//p' | sort | tr '\n' ' ' | sed 's/ $//'
}

expect() {
  local name="$1" want="$2" got
  shift 2
  total=$((total + 1))
  got="$(areas "$@")"
  if [ "$got" = "$want" ]; then
    echo "ok   $name"
  else
    echo "FAIL $name: want [$want], got [$got]"
    failures=$((failures + 1))
  fi
}

case_docs_only() {
  make_repo
  echo more >>docs/wiki/page.md
  commit
  expect "a docs change runs no area" "" main change
}

case_ui() {
  make_repo
  echo 'export const b = 1;' >>src/ui/a.ts
  commit
  expect "a UI change runs the web jobs and the Rust tests" "rust web" main change
}

case_rust_plain() {
  make_repo
  echo 'pub fn b() {}' >>src-tauri/src/core/a.rs
  commit
  expect "a Rust change with no OS code runs the Linux Rust jobs" "rust rust_src" main change
}

case_leak_path() {
  make_repo
  echo 'pub fn h() {}' >>src-tauri/src/net/gatekeeper.rs
  commit
  expect "a gatekeeper change runs the leak test" "leak rust rust_src" main change
}

case_macos_code() {
  make_repo
  echo 'pub fn n() {}' >>src-tauri/src/shell/menu.rs
  commit
  expect "a file with macOS code runs the macOS job" "macos rust rust_src" main change
}

case_macos_removed() {
  make_repo
  echo 'pub fn m() {}' >src-tauri/src/shell/menu.rs
  commit
  expect "removed macOS code runs the macOS job" "macos rust rust_src" main change
}

case_lock_file() {
  make_repo
  echo lock2 >src-tauri/Cargo.lock
  commit
  expect "a lock file change runs every OS, the leak test and the dependency checks" \
    "deps leak macos rust rust_src windows" main change
}

case_workflow() {
  make_repo
  echo 'on: pull_request' >.github/workflows/other.yml
  commit
  expect "a workflow change runs the workflow checks" "workflows" main change
}

case_pipeline() {
  make_repo
  echo 'on: push' >.github/workflows/ci.yml
  commit
  expect "a change to the fast lane runs every area" \
    "deps heavy leak macos rust rust_src web windows workflows" main change
}

case_all() {
  make_repo
  expect "--all runs every area" "deps heavy leak macos rust rust_src web windows workflows" --all
}

case_merge_base() {
  make_repo
  echo more >>docs/wiki/page.md
  commit
  git checkout -q main
  echo 'pub fn c() {}' >>src-tauri/src/core/a.rs
  commit
  git checkout -q change
  expect "a change on main after the branch started does not count" "" main change
}

case_issue_form() {
  make_repo
  mkdir -p .github/ISSUE_TEMPLATE
  echo 'name: Bug' >.github/ISSUE_TEMPLATE/bug.yml
  commit
  expect "the bug report form runs the Rust tests that read it" "rust workflows" main change
}

case_platform_shared() {
  make_repo
  mkdir -p src-tauri/crates/eepview-platform/src
  echo 'pub fn parse() {}' >src-tauri/crates/eepview-platform/src/input.rs
  commit
  expect "a shared file of the platform crate runs every OS" "leak macos rust rust_src windows" main change
}

case_deny_config() {
  make_repo
  echo '[bans]' >src-tauri/deny.toml
  commit
  expect "the cargo-deny config runs cargo-deny" "deps rust rust_src" main change
}

case_not_unix() {
  make_repo
  printf '#[cfg(not(unix))]\nfn no_follow() {}\n' >src-tauri/src/core/files.rs
  commit
  expect "code under not(unix) runs the Windows job" "rust rust_src windows" main change
}

case_any_windows() {
  make_repo
  printf '#[cfg(any(windows, target_os = "linux"))]\nfn w() {}\n' >src-tauri/src/core/w.rs
  commit
  expect "code under any(windows, ...) runs the Windows job" "rust rust_src windows" main change
}

case_navigation_rule() {
  make_repo
  mkdir -p src-tauri/src/core
  echo 'pub fn tab_navigation() {}' >src-tauri/src/core/page.rs
  commit
  expect "the navigation rule runs the leak test" "leak rust rust_src" main change
}

case_public_js() {
  make_repo
  mkdir -p public
  echo 'document.documentElement.dataset.theme = "dark";' >public/theme-boot.js
  commit
  expect "a script in public/ runs the web jobs and the Rust tests" "rust web" main change
}

case_heavy_lane() {
  make_repo
  echo 'on: schedule' >.github/workflows/heavy.yml
  commit
  expect "a change to the heavy lane runs it" "heavy workflows" main change
}

# The JSON list of runners for the macOS and Windows job.
expect_other_os() {
  local name="$1" want="$2" got
  shift 2
  total=$((total + 1))
  got="$("$root/scripts/ci-changes.sh" "$@" | sed -n 's/^other_os=//p')"
  if [ "$got" = "$want" ]; then
    echo "ok   $name"
  else
    echo "FAIL $name: want [$want], got [$got]"
    failures=$((failures + 1))
  fi
}

case_other_os() {
  make_repo
  echo more >>docs/wiki/page.md
  commit
  expect_other_os "no OS code gives the none runner" '["none"]' main change
  expect_other_os "--all gives both runners" '["macos-15","windows-2025"]' --all
  echo 'pub fn n() {}' >>src-tauri/src/shell/menu.rs
  commit
  expect_other_os "macOS code gives the macOS runner" '["macos-15"]' main change
}

case_docs_only
case_issue_form
case_platform_shared
case_deny_config
case_not_unix
case_any_windows
case_navigation_rule
case_public_js
case_heavy_lane
case_other_os
case_ui
case_rust_plain
case_leak_path
case_macos_code
case_macos_removed
case_lock_file
case_workflow
case_pipeline
case_all
case_merge_base

echo "${total} cases, ${failures} failed"
[ "$failures" -eq 0 ]
