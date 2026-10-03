#!/usr/bin/env bash
# Print the environment for a repeatable release build, one KEY=VALUE per line.
# Run it from the repo root. In CI: ./scripts/repro-env.sh >> "$GITHUB_ENV"
# - SOURCE_DATE_EPOCH is the commit time, so build tools stamp the same date.
# - RUSTFLAGS maps the home and checkout paths to fixed names, so the binary holds no build path.
set -euo pipefail

# rustc sees Windows paths (C:\...), not the /c/... paths of Git Bash.
native_path() {
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -w "$1"
  else
    printf '%s\n' "$1"
  fi
}

home="$(native_path "$HOME")"
checkout="$(native_path "$PWD")"

# rustc applies the last matching --remap-path-prefix, so the checkout wins over home.
echo "SOURCE_DATE_EPOCH=$(git log -1 --format=%ct)"
echo "CARGO_INCREMENTAL=0"
echo "RUSTFLAGS=--remap-path-prefix=$home=~ --remap-path-prefix=$checkout=."
