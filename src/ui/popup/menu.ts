// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import type { TabInfo } from "../contract.ts";
import { byId } from "../dom.ts";
import { call, on } from "../ipc.ts";
import { zoomText } from "../lib/tab-strip.ts";
import { closeCurrent } from "./frame.ts";

const quiet = (): undefined => undefined;

const COMMANDS: Record<string, () => Promise<unknown>> = {
  "new-tab": () => call("tab_new", {}),
  "zoom-in": () => call("zoom_in", {}),
  "zoom-out": () => call("zoom_out", {}),
  "zoom-reset": () => call("zoom_reset", {}),
};

function showZoom(tab: TabInfo | undefined): void {
  byId("zoom-value").textContent = zoomText(tab?.zoom ?? 1);
}

function showTabs(tabs: TabInfo[]): void {
  showZoom(tabs.find((t) => t.active));
}

export function openMenu(): void {
  call("tab_list", {}).then(showTabs).catch(quiet);
  byId("menu").querySelector<HTMLElement>("button")?.focus();
}

function onMenuClick(event: MouseEvent): void {
  const item = (event.target as Element).closest<HTMLElement>("[data-open],[data-command]");
  if (!item) return;
  const command = COMMANDS[item.dataset.command ?? ""];
  const open = item.dataset.open;
  if (command) command().catch(quiet);
  if (open) call("navigate", { input: open }).catch(quiet);
  if (open || item.dataset.command === "new-tab") closeCurrent(false);
}

export function wireMenu(): void {
  byId("menu").addEventListener("click", onMenuClick);
  on("tabs-changed", showTabs).catch(quiet);
  on("tab-updated", (tab) => {
    if (tab.active) showZoom(tab);
  }).catch(quiet);
}
