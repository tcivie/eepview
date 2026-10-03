#!/usr/bin/env bash
# Build the Linux release twice, in two different directories, and compare the outputs.
# Fails if the stripped binary or the frontend dist/ differ.
# Reports, but does not fail on, the .deb and the debug-symbol file.
# Usage: scripts/repro-check.sh <work-dir>
set -euo pipefail

target="x86_64-unknown-linux-gnu"
work="${1:?usage: repro-check.sh <work-dir>}"
summary="${GITHUB_STEP_SUMMARY:-/dev/stdout}"
status=0

build_in() {
  local dir="$1" key value
  git worktree add --detach "$dir" HEAD
  (
    cd "$dir"
    while IFS='=' read -r key value; do
      export "$key=$value"
    done < <(./scripts/repro-env.sh)
    npm ci --ignore-scripts
    npx tauri build --target "$target" --no-bundle -- --locked
    ./scripts/split-debug.sh "$target" symbols
    npx tauri bundle --target "$target" --bundles deb
    tar --sort=name --mtime="@$SOURCE_DATE_EPOCH" --owner=0 --group=0 --numeric-owner -cf dist.tar dist
  )
}

hash_of() {
  sha256sum "$1" | cut -d ' ' -f 1
}

# compare <label> <path inside each build dir> <fail|report>
compare() {
  local label="$1" path="$2" mode="$3" a b verdict
  a="$(hash_of "$work/a/$path")"
  b="$(hash_of "$work/b/$path")"
  verdict="identical"
  if [ "$a" != "$b" ]; then
    verdict="DIFFERENT"
    if [ "$mode" = "fail" ]; then
      status=1
    fi
  fi
  echo "| $label | \`${a:0:16}\` | \`${b:0:16}\` | $verdict |" >>"$summary"
}

mkdir -p "$work"
build_in "$work/a"
build_in "$work/b"

# Show what differs between the two .deb files, if diffoscope is installed.
explain_deb() {
  local a="$work/a/$1" b="$work/b/$1" report="$work/deb.diffoscope.txt"
  if [ "$(hash_of "$a")" = "$(hash_of "$b")" ] || ! command -v diffoscope >/dev/null 2>&1; then
    return 0
  fi
  if ! diffoscope --text "$report" "$a" "$b"; then
    echo "diffoscope: the .deb files differ. First 200 lines of $report:"
    head -n 200 "$report"
  fi
}

debs=("$work"/a/src-tauri/target/"$target"/release/bundle/deb/*.deb)
deb="${debs[0]#"$work/a/"}"
{
  echo "## Reproducible build ($target)"
  echo
  echo "| Output | Build A | Build B | Result |"
  echo "| --- | --- | --- | --- |"
} >>"$summary"
compare "binary (stripped)" "src-tauri/target/$target/release/eepview" fail
compare "frontend dist/ (tar)" "dist.tar" fail
compare "debug symbols" "symbols/eepview-$target.debug" report
compare ".deb" "$deb" report
explain_deb "$deb"

./scripts/check-hardening.sh "$work/a/src-tauri/target/$target/release/eepview"
exit "$status"
