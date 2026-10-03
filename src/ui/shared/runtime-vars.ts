// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

const sheets = new Map<string, CSSStyleSheet>();

function sheetFor(name: string): CSSStyleSheet {
  const known = sheets.get(name);
  if (known) return known;
  const sheet = new window.CSSStyleSheet();
  sheets.set(name, sheet);
  document.adoptedStyleSheets = [...document.adoptedStyleSheets, sheet];
  return sheet;
}

export function setRootVar(name: `--${string}`, value: string): void {
  sheetFor(name).replaceSync(`:root { ${name}: ${value}; }`);
}
