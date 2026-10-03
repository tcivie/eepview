// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import type { PopupShow, Suggestion, SuggestionsData } from "../contract.ts";
import { all, byId } from "../dom.ts";
import { call } from "../ipc.ts";
import { insetPx } from "../lib/chrome-height.ts";
import { setRootVar } from "../shared/runtime-vars.ts";
import { suggestionOption } from "../suggestion-option.ts";
import { closeCurrent } from "./frame.ts";

const list = (): HTMLElement => byId("suggestions");
let items: Suggestion[] = [];

export function renderSuggestions(show: PopupShow): void {
  const data = show.data as SuggestionsData | null;
  items = data?.items ?? [];
  const index = data?.index ?? -1;
  setRootVar("--popup-anchor-width", insetPx(show.anchorWidth));
  list().replaceChildren(...items.map((item, i) => suggestionOption(item, i, i === index)));
}

/** The toolbar moved the highlight: no new size, no new place. */
export function selectSuggestion(index: number): void {
  for (const option of all<HTMLElement>("[data-index]", list())) {
    option.setAttribute("aria-selected", String(Number(option.dataset.index) === index));
  }
}

/** A pick acts on mousedown, before the toolbar closes the list on blur. */
function onPick(event: MouseEvent): void {
  event.preventDefault();
  const option = (event.target as Element).closest<HTMLElement>("[data-index]");
  const item = items[Number(option?.dataset.index ?? -1)];
  if (!item) return;
  call("navigate", { input: item.url }).catch(() => undefined);
  closeCurrent(false);
}

export function wireSuggestions(): void {
  list().addEventListener("mousedown", onPick);
}
