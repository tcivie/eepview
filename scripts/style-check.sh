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
SKIP="$THEME" scan '^[^[:space:]/*}@]' "a rule outside @layer beats every layer, the [hidden] rule too" 'src/ui/*.css'
exit "$status"
