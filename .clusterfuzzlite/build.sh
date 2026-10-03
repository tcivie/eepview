#!/bin/bash -eu
# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
# Builds the cargo-fuzz targets of src-tauri/fuzz for ClusterFuzzLite.
# $SRC, $OUT and $SANITIZER come from the ClusterFuzzLite build image.

# The pinned nightly of the fuzz job. Only the fuzz job uses it; the app builds on the
# stable toolchain of rust-toolchain.toml. Keep this value equal to the one in
# docs/wiki/fuzzing.md.
NIGHTLY="nightly-2026-10-01"

cd "$SRC/eepview/src-tauri"
# tauri::generate_context! embeds the frontend folder; the fuzz targets need an empty one.
mkdir -p ../dist

rustup toolchain install "$NIGHTLY" --profile minimal
# The image does not ship cargo-fuzz. Build it without the sanitizer flags of the image.
env -u RUSTFLAGS cargo "+$NIGHTLY" install cargo-fuzz --version 0.13.2 --locked
cargo "+$NIGHTLY" fuzz build -O --sanitizer="$SANITIZER" --target-dir "$SRC/target"

for dict in fuzz/dictionaries/*.dict; do
  cp "$dict" "$OUT/"
done
for target in fuzz/fuzz_targets/*.rs; do
  name="$(basename "$target" .rs)"
  cp "$SRC/target/x86_64-unknown-linux-gnu/release/$name" "$OUT/$name"
done
