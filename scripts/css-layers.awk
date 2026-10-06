# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT
#
# Every style rule must sit inside an @layer that theme.css declares. A rule outside a layer, or in
# an undeclared layer (which CSS appends after the last one), beats `[hidden]` in @layer state.
# Allowed at the top level: the `@layer a, b;` order statement, `:root` token blocks, and an
# `@media` or `@supports` block that holds only `:root` blocks.
#
# Usage: awk -v layers="base components pages state" -f scripts/css-layers.awk file.css ...
# Prints one "file:line: reason" per fault and exits 1 when there is one.

BEGIN {
  n = split(layers, names, " ")
  for (i = 1; i <= n; i++) declared[names[i]] = 1
}

FNR == 1 {
  depth = 0
  prelude = ""
  in_comment = 0
}

function trim(s) {
  gsub(/^[[:space:]]+|[[:space:]]+$/, "", s)
  return s
}

function fault(reason) {
  printf "%s:%d: %s\n", FILENAME, start, reason
  faults++
}

function open_block(p, kind, name) {
  kind = "other"
  if (depth == 0) {
    if (p ~ /^@layer[[:space:]]+[A-Za-z0-9_-]+$/) {
      name = p
      sub(/^@layer[[:space:]]+/, "", name)
      if (name in declared) {
        kind = "layer"
      } else {
        fault("@layer " name " is not in the order that theme.css declares")
      }
    } else if (p ~ /^:root/) {
      kind = "root"
    } else if (p ~ /^@(media|supports)/) {
      kind = "wrap"
    } else {
      fault("rule outside @layer: " p)
    }
  } else if (stack[depth] == "wrap" && p !~ /^:root/) {
    fault("only :root blocks may sit in a top-level " "@media or @supports: " p)
  }
  stack[++depth] = kind
}

{
  line = $0
  for (i = 1; i <= length(line); i++) {
    c = substr(line, i, 1)
    two = substr(line, i, 2)
    if (in_comment) {
      if (two == "*/") { in_comment = 0; i++ }
      continue
    }
    if (two == "/*") { in_comment = 1; i++; continue }
    if (c == "{") {
      open_block(trim(prelude))
      prelude = ""
    } else if (c == "}") {
      if (depth > 0) depth--
      prelude = ""
    } else if (c == ";") {
      p = trim(prelude)
      if (depth == 0 && p !~ /^@layer[[:space:]]+[A-Za-z0-9_, -]+$/) {
        fault("statement outside @layer: " p)
      }
      prelude = ""
    } else {
      if (trim(prelude) == "" && c !~ /[[:space:]]/) start = FNR
      prelude = prelude c
    }
  }
  prelude = prelude " "
}

END { exit faults > 0 }
