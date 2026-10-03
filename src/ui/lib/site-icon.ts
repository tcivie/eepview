// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

export const ICON_PREFIX = "data:image/png;base64,";

export type SiteMark = { kind: "icon"; src: string } | { kind: "letter"; text: string };

const BASE64 = /^[A-Za-z0-9+/]+={0,2}$/;

function isIcon(icon: string | null | undefined): icon is string {
  if (typeof icon !== "string" || !icon.startsWith(ICON_PREFIX)) return false;
  return BASE64.test(icon.slice(ICON_PREFIX.length));
}

/** The icon when it is a valid PNG data URL, or the letter chip. */
export function siteMark(icon: string | null | undefined, letter: string): SiteMark {
  return isIcon(icon) ? { kind: "icon", src: icon } : { kind: "letter", text: letter };
}
