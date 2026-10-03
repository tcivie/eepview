#!/usr/bin/env bash
# Build eepview-router-helper.jar against the jars of an installed I2P router.
# Usage: router-helper/build.sh <i2p-base-dir>   (the dir that holds lib/router.jar)
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <i2p-base-dir>" >&2
  exit 2
fi

i2p_base=$1
here=$(cd "$(dirname "$0")" && pwd)
out="$here/out"
lib="$i2p_base/lib"

if [[ ! -f "$lib/router.jar" ]]; then
  echo "no router.jar in $lib" >&2
  exit 1
fi

rm -rf "$out"
mkdir -p "$out/classes"
find "$here/src/main/java" -name '*.java' > "$out/sources.txt"
javac --release 17 -Xlint:all,-path -Werror \
  -cp "$lib/*" -d "$out/classes" @"$out/sources.txt"
jar cf "$out/eepview-router-helper.jar" -C "$out/classes" .
echo "$out/eepview-router-helper.jar"
