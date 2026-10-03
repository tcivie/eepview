// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// Router console quick links: buttons that call console_open. No page holds a link to the
// console address itself.
import type { ConsoleInfo, ConsolePage } from "./contract.ts";
import { call } from "./ipc.ts";
import { type ConsoleLinkView, consoleLinks } from "./lib/console-links.ts";

export interface ConsoleSlots {
  list: HTMLElement;
  note: HTMLElement;
  title?: HTMLElement;
}

const quiet = (): undefined => undefined;

function linkButton(link: ConsoleLinkView): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "btn console-link";
  button.dataset.consolePage = link.page;
  button.textContent = link.label;
  return button;
}

export function renderConsoleLinks(
  slots: ConsoleSlots,
  info: ConsoleInfo | null,
  only?: readonly ConsolePage[],
): void {
  const view = consoleLinks(info);
  const links = only ? view.links.filter((link) => only.includes(link.page)) : view.links;
  slots.list.replaceChildren(...links.map(linkButton));
  slots.list.hidden = links.length === 0;
  slots.note.textContent = view.note ?? "";
  slots.note.hidden = view.found;
  if (slots.title) slots.title.textContent = view.title ?? "Router console";
}

export function wireConsoleClicks(root: HTMLElement, after?: () => void): void {
  root.addEventListener("click", (event) => {
    const target = (event.target as Element).closest<HTMLElement>("[data-console-page]");
    const page = target?.dataset.consolePage as ConsolePage | undefined;
    if (!page) return;
    call("console_open", { page }).catch(quiet);
    after?.();
  });
}

// True when the active tab shows this internal page: only then does a page probe for the
// console (docs/wiki/router-console.md, R6).
export async function shownInActiveTab(page: string): Promise<boolean> {
  const tabs = await call("tab_list", {});
  const active = tabs.find((tab) => tab.active);
  return active?.kind === "internal" && active.url.startsWith(`eepview://${page}`);
}
