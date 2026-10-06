#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
set -euo pipefail

cd "$(dirname "$0")/.."

THEME="src/ui/theme.css"
SIDES='(-(top|right|bottom|left|inline|block)(-(start|end))?)?'
CORNERS='(-(top|bottom|start|end)-(left|right|start|end))?'
PROPS="(margin${SIDES}|padding${SIDES}|gap|row-gap|column-gap|border${CORNERS}-radius|font-size|font)"
UNITS='(px|rem|em|ex|ch|lh|pt|pc|cm|mm|in|q|vw|vh|vmin|vmax|dvh|svh|lvh|%)'
RAW="^[^:]+:[0-9]+:[[:space:]]*${PROPS}[[:space:]]*:.*([^-a-z0-9]|^)[0-9]*[.]?[0-9]+${UNITS}([^a-z]|$)"
VAR='var\(--[a-z0-9-]+\)'
TOKEN_DEF='^[[:space:]]*--[a-z0-9-]+[[:space:]]*:'
HTML_STYLE='<style([[:space:]>]|$)|[[:space:]]style[[:space:]]*='
TS_STYLE="\\.style([^A-Za-z0-9_]|$)|cssText|setAttribute\\([[:space:]]*.style|[[:space:]]style[[:space:]]*=|<style"
status=0

fail() {
  echo "error: $*" >&2
  status=1
}

report() {
  local label="$1" line
  while IFS= read -r line; do
    fail "${label}: ${line}"
  done
}

scan() {
  local pattern="$1" label="$2" file files=()
  shift 2
  while IFS= read -r file; do
    files+=("$file")
  done < <(git ls-files -- "$@" | grep -vxF "${SKIP:-}" || true)
  [ "${#files[@]}" -eq 0 ] && return 0
  report "$label" < <(grep -nHiE "$pattern" "${files[@]}" || true)
}

declarations() {
  awk -v file="$1" '
    buf == "" && /^[[:space:]]*[a-z-]+[[:space:]]*:/ && !/[{,][[:space:]]*$/ { start = FNR; buf = " " }
    buf != "" { buf = buf " " $0 }
    buf != "" && /;/ { print file ":" start ":" buf; buf = "" }
  ' "$1"
}

raw_values() {
  local file
  while IFS= read -r file; do
    declarations "$file" | sed -E -e 's/^([^:]+:[0-9]+:)[[:space:]]+/\1 /' -e "s/${VAR}//g" | grep -iE "$RAW" || true
  done < <(git ls-files -- '*.css')
}

report "spacing, radius and font size must come from a token" < <(raw_values)
SKIP="$THEME" scan "$TOKEN_DEF" "define tokens only in ${THEME}" '*.css'
scan "$HTML_STYLE" "no inline style in HTML" '*.html'
scan "$TS_STYLE" "no inline style in code" 'src/*.ts' 'scripts/*.mjs'
LAYERS="$(sed -nE 's/^@layer ([a-z, -]+);$/\1/p' "$THEME" | tr -d ',')"
[ -n "$LAYERS" ] || fail "${THEME} must declare the layer order (@layer a, b, c;)"
# awk exits 1 when it found a fault. Any other non-zero exit is a crash, and a crash must fail the
# gate: it prints only to stderr, so the report would read nothing and pass. awk runs once, without
# xargs, because xargs turns every exit code into the same one.
css_layers() {
  local code=0 file sheets=()
  while IFS= read -r -d '' file; do
    sheets+=("$file")
  done < <(git ls-files -z -- '*.css')
  awk -v layers="$LAYERS" -f scripts/css-layers.awk "${sheets[@]}" || code=$?
  if [ "$code" -gt 1 ]; then
    echo "scripts/css-layers.awk: awk stopped with exit ${code}, so the layer check did not run"
  fi
}
report "every rule must sit in a layer that ${THEME} declares" < <(css_layers)
exit "$status"
