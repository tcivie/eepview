// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import type { HintData, PopupClosed, PopupKind, SuggestionsData } from "../contract.ts";
import { call, on } from "../ipc.ts";
import { clickOpens } from "../lib/popup-toggle.ts";

interface Open {
  kind: PopupKind;
  id: Promise<number>;
  known: number | null;
}

type Closed = (refocus: boolean) => void;

const quiet = (): undefined => undefined;
const closedAt: Partial<Record<PopupKind, number>> = {};
const listeners: Partial<Record<PopupKind, Closed>> = {};
let open: Open | null = null;

export function openKind(): PopupKind | null {
  return open?.kind ?? null;
}

/** Runs `listener` when the popup of `kind` closes, from here or from the popup page. */
export function onPopupClosed(kind: PopupKind, listener: Closed): void {
  listeners[kind] = listener;
}

function ended(kind: PopupKind, refocus: boolean): void {
  closedAt[kind] = Date.now();
  listeners[kind]?.(refocus);
}

/** Opens a popup under `anchor`; another kind that was open closes first (in the shell). */
export function openPopup(
  kind: PopupKind,
  anchor: Element,
  data?: SuggestionsData | HintData,
): void {
  const previous = open?.kind;
  const r = anchor.getBoundingClientRect();
  const rect = { x: r.left, y: r.top, width: r.width, height: r.height };
  const entry: Open = { kind, id: call("popup_open", { kind, anchor: rect, data }), known: null };
  open = entry;
  entry.id.then((id) => {
    entry.known = id;
  }, quiet);
  if (previous && previous !== kind) ended(previous, false);
}

/** Closes the popup of `kind`, by its id, once the id is known. */
export function closePopup(kind: PopupKind): void {
  if (open?.kind !== kind) return;
  const { id } = open;
  open = null;
  id.then((known) => call("popup_close", { id: known })).catch(quiet);
  ended(kind, false);
}

/** The shell closed a popup (Esc, a click outside, a resize, a pick in the popup). */
function onShellClosed(closed: PopupClosed): void {
  if (open?.kind !== closed.kind || open.known !== closed.id) return;
  open = null;
  ended(closed.kind, closed.refocus);
}

/**
 * Makes `button` toggle the popup of `kind`. The state is read at `pointerdown`: when the
 * popup was open at that moment, the click closes it, however long the press lasts.
 */
export function wireToggle(button: HTMLElement, kind: PopupKind, show: () => void): void {
  let press: { open: PopupKind | null; closedAt: number | undefined; at: number } | null = null;
  const snapshot = () => ({ open: openKind(), closedAt: closedAt[kind], at: Date.now() });
  button.addEventListener("pointerdown", () => {
    press = snapshot();
  });
  button.addEventListener("click", () => {
    const state = press ?? snapshot();
    press = null;
    if (clickOpens(state.open, state.closedAt, kind, state.at)) show();
    else closePopup(kind);
  });
}

export function wirePopups(): void {
  on("popup-closed", onShellClosed).catch(quiet);
}
