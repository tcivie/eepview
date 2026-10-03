// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// The router console link: one button that calls console_open, which opens the console
// home page in the console tab. No page holds a link to the console address itself.
import type { ConsoleInfo } from "./contract.ts";
import { call } from "./ipc.ts";
import { consoleLink } from "./lib/console-links.ts";

export interface ConsoleSlots {
  list: HTMLElement;
  note: HTMLElement;
  title?: HTMLElement;
}

const quiet = (): undefined => undefined;

function linkButton(label: string): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "btn console-link";
  button.dataset.consoleOpen = "";
  button.textContent = label;
  return button;
}

export function renderConsoleLink(slots: ConsoleSlots, info: ConsoleInfo | null): void {
  const view = consoleLink(info);
  slots.list.replaceChildren(...(view.label ? [linkButton(view.label)] : []));
  slots.list.hidden = !view.found;
  slots.note.textContent = view.note ?? "";
  slots.note.hidden = view.found;
  if (slots.title) slots.title.textContent = view.title ?? "Router console";
}

export function wireConsoleClicks(root: HTMLElement, after?: () => void): void {
  root.addEventListener("click", (event) => {
    if (!(event.target as Element).closest("[data-console-open]")) return;
    call("console_open", {}).catch(quiet);
    after?.();
  });
}

// True when the active tab shows this internal page: only then does a page probe for the
// console (R6 of the router console page in docs/wiki).
export async function shownInActiveTab(page: string): Promise<boolean> {
  const tabs = await call("tab_list", {});
  const active = tabs.find((tab) => tab.active);
  return active?.kind === "internal" && active.url.startsWith(`eepview://${page}`);
}
