// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import type { PopupKind, PopupShow } from "../contract.ts";
import { byId } from "../dom.ts";
import { call } from "../ipc.ts";

const CARDS: Record<PopupKind, string> = {
  suggestions: "suggestions",
  menu: "menu",
  router: "router-panel",
  hint: "hint",
};
const quiet = (): undefined => undefined;
let current: PopupShow | null = null;

export function currentPopup(): PopupShow | null {
  return current;
}

export function cardOf(kind: PopupKind): HTMLElement {
  return byId(CARDS[kind]);
}

/** Shows the card of `show.kind` and hides the others. */
export function showCard(show: PopupShow): void {
  current = show;
  for (const [kind, id] of Object.entries(CARDS)) byId(id).hidden = kind !== show.kind;
}

/** Tells the shell the natural size of the open card, measured with no size limit. */
export function report(): void {
  if (!current) return;
  const root = document.documentElement;
  root.classList.add("measuring");
  const rect = cardOf(current.kind).getBoundingClientRect();
  root.classList.remove("measuring");
  const size = { width: Math.ceil(rect.width), height: Math.ceil(rect.height) };
  call("popup_size", { id: current.id, ...size }).catch(quiet);
}

/** Asks the shell to close the open popup; `refocus` gives the focus back to the toolbar. */
export function closeCurrent(refocus: boolean): void {
  if (current) call("popup_close", { id: current.id, refocus }).catch(quiet);
}

/** The shell closed popup `id`: hide its card. Returns its kind when it was the open one. */
export function forget(id: number): PopupKind | null {
  if (current?.id !== id) return null;
  const kind = current.kind;
  cardOf(kind).hidden = true;
  current = null;
  return kind;
}
