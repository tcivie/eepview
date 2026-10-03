#!/bin/bash -eu
# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
# Builds the cargo-fuzz targets of src-tauri/fuzz for ClusterFuzzLite.
# $SRC, $OUT and $SANITIZER come from the ClusterFuzzLite build image.

# The pinned nightly. One file names it for this build and for the branch-coverage job. The
# app builds on the stable toolchain of rust-toolchain.toml.
NIGHTLY="$(cat "$SRC/eepview/scripts/nightly-toolchain.txt")"

# The glibc parts that the runner image already has.
GLIBC_LIBS='/(ld-linux[^/]*|libc|libm|libdl|libpthread|librt|libresolv|libutil)\.so'

# The targets link the GTK and WebKitGTK libraries that the Tauri crates use, and the runner
# image does not have them. Copy every shared library of a target next to it. The rpath
# makes the binary and its libraries look in their own directory first.
bundle_libs() {
  local name="$1"
  ldd "$OUT/$name" | awk '/=> \//{print $3}' | grep -Ev "$GLIBC_LIBS" |
    while read -r lib; do
      cp -n "$lib" "$OUT/"
    done
}

cd "$SRC/eepview/src-tauri"
# tauri::generate_context! embeds the frontend folder; the fuzz targets need an empty one.
mkdir -p ../dist
# Fuzz the dependency versions that the app ships: start from the lock file of the app.
cp Cargo.lock fuzz/Cargo.lock

rustup toolchain install "$NIGHTLY" --profile minimal
# The image does not ship cargo-fuzz. Build it without the sanitizer flags of the image.
env -u RUSTFLAGS cargo "+$NIGHTLY" install cargo-fuzz --version 0.13.2 --locked

export RUSTFLAGS="${RUSTFLAGS:-} -Clink-arg=-Wl,--disable-new-dtags -Clink-arg=-Wl,-rpath,\$ORIGIN"
cargo "+$NIGHTLY" fuzz build -O --sanitizer="$SANITIZER" --target-dir "$SRC/target"

cp fuzz/dictionaries/host.dict fuzz/dictionaries/gatekeeper_request.dict "$OUT/"
# The two URL targets share one dictionary.
cp fuzz/dictionaries/url.dict "$OUT/address_bar.dict"
cp fuzz/dictionaries/url.dict "$OUT/url_rules.dict"
for target in fuzz/fuzz_targets/*.rs; do
  name="$(basename "$target" .rs)"
  cp "$SRC/target/x86_64-unknown-linux-gnu/release/$name" "$OUT/$name"
  bundle_libs "$name"
done
du -sh "$OUT"
