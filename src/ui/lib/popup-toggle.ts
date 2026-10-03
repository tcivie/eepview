// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

export type PopupKind = "suggestions" | "menu" | "router" | "hint";

/** A popup that closed this recently was closed by the same click on its button. */
export const TOGGLE_MS = 300;

/**
 * True when a click on the button of `kind` opens it: that popup is not open, and it did
 * not close less than `TOGGLE_MS` before `now` (the press on the button closed it).
 */
export function clickOpens(
  open: PopupKind | null,
  closedAt: number | undefined,
  kind: PopupKind,
  now: number,
): boolean {
  if (open === kind) return false;
  return closedAt === undefined || now - closedAt >= TOGGLE_MS;
}
