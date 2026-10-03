// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import type { Suggestion } from "./contract.ts";
import { displayUrl } from "./lib/address.ts";

/** One suggestion row; `selected` marks the highlighted one. */
export function suggestionOption(item: Suggestion, index: number, selected: boolean): HTMLElement {
  const option = document.createElement("div");
  option.id = `suggestion-${index}`;
  option.className = "suggestion";
  option.setAttribute("role", "option");
  option.setAttribute("aria-selected", String(selected));
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
