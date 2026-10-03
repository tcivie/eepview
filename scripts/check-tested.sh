#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
#
# Traceability check: every written requirement has at least one test.
# A test names the requirement it checks with a tag in its title:
#   [browser-ui N]             numbered requirement N in docs/wiki/browser-ui.md
#   [ipc-contract <section>]   a section of the IPC contract, listed in IPC_SECTIONS
# The script fails when a requirement has no test that names it.
# Usage: scripts/check-tested.sh
set -euo pipefail

cd "$(dirname "$0")/.."

# IPC contract sections that the UI depends on, by their anchor in docs/wiki/ipc-contract.md.
# history-1 is the History section under Commands (the first History heading is the change log).
IPC_SECTIONS=(
  navigation-active-tab
  window-layout
  window
  history-1
  events
  keyboard-shortcuts
)

# Print the number of each numbered item under "## Requirements" in browser-ui.md.
browser_ui_numbers() {
  awk '/^## /{inside = ($0 == "## Requirements")} inside && /^[0-9]+\. /{sub(/\..*/, ""); print}' \
    docs/wiki/browser-ui.md
}

# Print every tag that a test file in src/ names.
test_tags() {
  find src -name '*.test.ts' -print0 |
    xargs -0 grep -ohE '\[(browser-ui [0-9]+|ipc-contract [a-z0-9-]+)\]' |
    sort -u
}

main() {
  local tags status=0 number section
  tags="$(test_tags || true)"
  while IFS= read -r number; do
    if ! grep -qxF "[browser-ui ${number}]" <<<"$tags"; then
      echo "error: browser-ui.md requirement ${number} has no test tagged [browser-ui ${number}]." >&2
      status=1
    fi
  done < <(browser_ui_numbers)
  for section in "${IPC_SECTIONS[@]}"; do
    if ! grep -qxF "[ipc-contract ${section}]" <<<"$tags"; then
      echo "error: IPC contract section ${section} has no test tagged [ipc-contract ${section}]." >&2
      status=1
    fi
  done
  return "$status"
}

main
