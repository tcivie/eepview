// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

export const CHROME_HEIGHT = 84;
export const FIND_BAR_HEIGHT = 40;

export interface ChromeLayout {
  findOpen: boolean;
}

export function insetPx(left: number): string {
  return `${Number.isFinite(left) ? Math.max(0, Math.round(left)) : 0}px`;
}

/** The toolbar height: 84, or 124 with the find bar. Popups never change it. */
export function chromeHeight(layout: ChromeLayout): number {
  return CHROME_HEIGHT + (layout.findOpen ? FIND_BAR_HEIGHT : 0);
}
