// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import type { FindResult } from "../contract.ts";
import { byId } from "../dom.ts";
import { call } from "../ipc.ts";
import { findCountText, findTone } from "../lib/find-text.ts";
import { bindClicks, keyActions } from "../shared/events.ts";

const quiet = (): undefined => undefined;
const bar = (): HTMLFormElement => byId("findbar");
const input = (): HTMLInputElement => byId("find-input");
const caseButton = (): HTMLElement => byId("find-case");

function matchCase(): boolean {
  return caseButton().getAttribute("aria-pressed") === "true";
}

export function showFindResult(result: FindResult | null): void {
  const count = byId("find-count");
  count.textContent = findCountText(result);
  count.dataset.tone = findTone(result);
}

function runFind(forward: boolean): void {
  const query = input().value;
  if (query === "") {
    showFindResult(null);
    return;
  }
  call("find", { query, forward, matchCase: matchCase() }).catch(quiet);
}

export function openFind(): void {
  bar().hidden = false;
  document.body.classList.add("find-open");
  input().focus();
  input().select();
}

export function closeFind(): void {
  if (bar().hidden) return;
  bar().hidden = true;
  document.body.classList.remove("find-open");
  call("find_close", {}).catch(quiet);
}

const onKeyDown = keyActions<KeyboardEvent>({
  Enter: (event) => runFind(!event.shiftKey),
  Escape: () => closeFind(),
});

function toggleCase(): void {
  caseButton().setAttribute("aria-pressed", String(!matchCase()));
  runFind(true);
}

export function wireFind(): void {
  input().addEventListener("input", () => runFind(true));
  input().addEventListener("keydown", onKeyDown);
  bindClicks(
    {
      "find-next": () => runFind(true),
      "find-prev": () => runFind(false),
      "find-case": toggleCase,
      "find-close": closeFind,
    },
    byId,
  );
  bar().addEventListener("submit", (e) => e.preventDefault());
}
