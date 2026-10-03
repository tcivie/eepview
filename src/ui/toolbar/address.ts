// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import type { Suggestion, TabInfo } from "../contract.ts";
import { byId } from "../dom.ts";
import { call, tell } from "../ipc.ts";
import { addressBadge, displayUrl } from "../lib/address.ts";
import {
  emptySuggest,
  type KeyOutcome,
  onKey,
  type SuggestKey,
  type SuggestState,
  withItems,
} from "../lib/suggest-state.ts";
import { keyActions } from "../shared/events.ts";
import { suggestionOption } from "../suggestion-option.ts";
import { closePopup, onPopupClosed, openKind, openPopup } from "./popups.ts";
import { activeTab } from "./state.ts";

let state: SuggestState<Suggestion> = emptySuggest();
let editing = false;
let requestSeq = 0;

const input = (): HTMLInputElement => byId("address");
const list = (): HTMLElement => byId("suggestions");

export function showUrl(tab: TabInfo | undefined): void {
  const url = tab?.url ?? "";
  const badge = addressBadge(tab);
  const el = byId("address-badge");
  el.hidden = !badge;
  if (badge) {
    el.textContent = badge.text;
    el.title = badge.title;
    el.dataset.kind = badge.kind;
  }
  if (!editing) input().value = displayUrl(url);
}

const BLUR_CLOSE_MS = 150;
let shown: Suggestion[] | null = null;
let blurTimer = 0;

/** The visible list lives in the popup webview; this hidden listbox serves screen readers. */
function renderA11y(open: boolean): void {
  const items = open ? state.items : [];
  list().replaceChildren(...items.map((item, i) => suggestionOption(item, i, i === state.index)));
  list().hidden = !open;
  input().setAttribute("aria-expanded", String(open));
  const active = open && state.index >= 0 ? `suggestion-${state.index}` : "";
  if (active) input().setAttribute("aria-activedescendant", active);
  else input().removeAttribute("aria-activedescendant");
}

/** Shows the list in the popup; a highlight move only tells the popup (no new size). */
function renderPopup(open: boolean): void {
  if (!open) {
    shown = null;
    closePopup("suggestions");
  } else if (shown === state.items && openKind() === "suggestions") {
    tell("popup-select", { index: state.index }).catch(() => undefined);
  } else {
    shown = state.items;
    openPopup("suggestions", byId("address-form"), { items: state.items, index: state.index });
  }
}

function renderList(): void {
  const open = state.open && state.items.length > 0;
  renderA11y(open);
  renderPopup(open);
}

function closeList(): void {
  requestSeq += 1;
  state = emptySuggest();
  renderList();
}

function go(text: string): void {
  editing = false;
  closeList();
  input().blur();
  if (text.trim() !== "") call("navigate", { input: text }).catch(() => undefined);
}

function revert(): void {
  editing = false;
  closeList();
  showUrl(activeTab());
  input().blur();
}

function applyOutcome(outcome: KeyOutcome<Suggestion>): void {
  state = outcome.state;
  renderList();
  if (outcome.revert) revert();
  if (outcome.go) go(outcome.go === "typed" ? input().value : outcome.go.url);
}

const handleKey = (key: SuggestKey) => () => applyOutcome(onKey(state, key));

const onKeyDown = keyActions<KeyboardEvent>({
  ArrowDown: handleKey("ArrowDown"),
  ArrowUp: handleKey("ArrowUp"),
  Escape: handleKey("Escape"),
  Enter: handleKey("Enter"),
});

function onInput(): void {
  editing = true;
  const seq = ++requestSeq;
  const text = input().value;
  call("suggest", { input: text })
    .then((items) => {
      if (seq !== requestSeq) return;
      state = withItems(items);
      renderList();
    })
    .catch(() => undefined);
}

/** The address field lost focus: close the list a moment later, so a pick lands first. */
function leaveField(): void {
  editing = false;
  closeList();
}

/** The popup closed the list (a pick, Esc, a resize): leave editing. */
function onPopupGone(): void {
  if (!state.open) return;
  editing = false;
  state = emptySuggest();
  shown = null;
  renderA11y(false);
  showUrl(activeTab());
}

export function focusAddress(): void {
  input().focus();
  input().select();
}

export function wireAddress(): void {
  const field = input();
  field.addEventListener("focus", () => field.select());
  field.addEventListener("keydown", onKeyDown);
  field.addEventListener("input", onInput);
  field.addEventListener("focus", () => window.clearTimeout(blurTimer));
  field.addEventListener("blur", () => {
    window.clearTimeout(blurTimer);
    blurTimer = window.setTimeout(leaveField, BLUR_CLOSE_MS);
  });
  onPopupClosed("suggestions", onPopupGone);
  byId<HTMLFormElement>("address-form").addEventListener("submit", (e) => e.preventDefault());
}

export function previewSuggestions(text: string): void {
  input().focus();
  input().value = text;
  onInput();
}
