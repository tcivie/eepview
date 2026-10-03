#!/usr/bin/env bash
# Check that a Linux ELF binary is PIE, has full RELRO and a non-executable stack.
# Prints the readelf lines it checks. Fails if one is missing.
# Usage: scripts/check-hardening.sh <elf-file>
set -euo pipefail

elf="${1:?usage: check-hardening.sh <elf-file>}"
status=0

check() {
  local label="$1" pattern="$2" text="$3" line
  if line="$(grep -E "$pattern" <<<"$text")"; then
    echo "ok   $label: $(tr -s ' ' <<<"$line" | head -n 1)"
  else
    echo "FAIL $label" >&2
    status=1
  fi
}

header="$(readelf -hW "$elf")"
segments="$(readelf -lW "$elf")"
dynamic="$(readelf -dW "$elf")"

check "PIE (ELF type)" 'Type:[[:space:]]+DYN' "$header"
check "PIE (FLAGS_1)" 'FLAGS_1.*PIE' "$dynamic"
check "RELRO segment" 'GNU_RELRO' "$segments"
check "BIND_NOW (full RELRO)" '(BIND_NOW|FLAGS.*[[:space:]]NOW)' "$dynamic"
check "NX stack (GNU_STACK RW, not RWE)" 'GNU_STACK([[:space:]]+[^[:space:]]+){5}[[:space:]]+RW[[:space:]]' "$segments"

exit "$status"
