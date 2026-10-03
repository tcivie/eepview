// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

export const CHROME_HEIGHT = 84;
export const FIND_BAR_HEIGHT = 40;
export const POPUP_MARGIN = 8;

export interface ChromeLayout {
  findOpen: boolean;
  popupBottoms: number[];
}

export function insetPx(left: number): string {
  return `${Number.isFinite(left) ? Math.max(0, Math.round(left)) : 0}px`;
}

export function chromeHeight(layout: ChromeLayout): number {
  const base = CHROME_HEIGHT + (layout.findOpen ? FIND_BAR_HEIGHT : 0);
  const popups = layout.popupBottoms.map((bottom) => Math.ceil(bottom) + POPUP_MARGIN);
  return Math.max(base, ...popups);
}
