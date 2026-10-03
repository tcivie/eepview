// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import type { Suggestion, TabInfo } from "../contract.ts";
import { byId } from "../dom.ts";
import { call } from "../ipc.ts";
import { displayUrl, isInternal } from "../lib/address.ts";
import {
  emptySuggest,
  type KeyOutcome,
  onKey,
  type SuggestKey,
  type SuggestState,
  withItems,
} from "../lib/suggest-state.ts";
import { keyActions } from "../shared/events.ts";
import { activeTab } from "./state.ts";

let state: SuggestState<Suggestion> = emptySuggest();
let editing = false;
let requestSeq = 0;

const input = (): HTMLInputElement => byId("address");
const list = (): HTMLElement => byId("suggestions");

export function showUrl(tab: TabInfo | undefined): void {
  const url = tab?.url ?? "";
  byId("address-badge").hidden = !tab || isInternal(url);
  if (!editing) input().value = displayUrl(url);
}

function optionFor(item: Suggestion, index: number): HTMLElement {
  const option = document.createElement("div");
  option.id = `suggestion-${index}`;
  option.className = "suggestion";
  option.setAttribute("role", "option");
  option.setAttribute("aria-selected", String(index === state.index));
  option.dataset.index = String(index);
  option.dataset.source = item.source;
  const title = document.createElement("span");
  title.className = "suggestion-title";
  title.textContent = item.title || displayUrl(item.url);
  const url = document.createElement("span");
  url.className = "suggestion-url";
  url.textContent = displayUrl(item.url);
  option.append(title, url);
  return option;
}

function renderList(): void {
  const open = state.open && state.items.length > 0;
  list().replaceChildren(...(open ? state.items.map(optionFor) : []));
  list().hidden = !open;
  input().setAttribute("aria-expanded", String(open));
  const active = open && state.index >= 0 ? `suggestion-${state.index}` : "";
  if (active) input().setAttribute("aria-activedescendant", active);
  else input().removeAttribute("aria-activedescendant");
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

function onOptionPick(event: MouseEvent): void {
  event.preventDefault();
  const option = (event.target as Element).closest<HTMLElement>("[data-index]");
  const item = state.items[Number(option?.dataset.index ?? -1)];
  if (item) go(item.url);
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
  field.addEventListener("blur", () => {
    editing = false;
    closeList();
  });
  list().addEventListener("mousedown", onOptionPick);
  byId<HTMLFormElement>("address-form").addEventListener("submit", (e) => e.preventDefault());
}

export function previewSuggestions(text: string): void {
  input().focus();
  input().value = text;
  onInput();
}
