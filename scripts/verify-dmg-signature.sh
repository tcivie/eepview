#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
# Mount each dmg in a folder and check that the .app inside is ad-hoc signed and lets its web
# views load http:// eepsites (App Transport Security, docs/wiki/browser-shell.md B1).
# Usage: verify-dmg-signature.sh <dmg-dir>
set -euo pipefail

dir="${1:?usage: verify-dmg-signature.sh <dmg-dir>}"

verify_dmg() {
  local dmg="$1" mnt app info
  mnt="$(mktemp -d)"
  hdiutil attach -nobrowse -readonly -mountpoint "$mnt" "$dmg" >/dev/null
  # Detach on every exit path of this function.
  trap 'hdiutil detach "$mnt" >/dev/null || true' RETURN
  app="$(find "$mnt" -maxdepth 1 -name '*.app' | head -n 1)"
  if [ -z "$app" ]; then
    echo "::error::no .app inside $dmg"
    return 1
  fi
  codesign --verify --deep --strict --verbose=2 "$app"
  info="$(codesign -dv "$app" 2>&1)"
  printf '%s\n' "$info"
  if ! grep -q 'Signature=adhoc' <<<"$info"; then
    echo "::error::$app is not ad-hoc signed"
    return 1
  fi
  verify_ats "$app"
}

# Without this key, ATS refuses every http:// page load and no eepsite loads in a tab.
verify_ats() {
  local app="$1" key=NSAppTransportSecurity.NSAllowsArbitraryLoadsInWebContent value
  value="$(plutil -extract "$key" raw "$app/Contents/Info.plist" 2>/dev/null || true)"
  if [ "$value" != "true" ]; then
    echo "::error::$app: $key is not true in Info.plist"
    return 1
  fi
}

found=0
for dmg in "$dir"/*.dmg; do
  [ -e "$dmg" ] || continue
  found=1
  verify_dmg "$dmg"
done
if [ "$found" -eq 0 ]; then
  echo "::error::no dmg in $dir"
  exit 1
fi
