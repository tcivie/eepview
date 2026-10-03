import type { TabInfo } from "../contract.ts";

let tabs: TabInfo[] = [];
const listeners = new Set<(tabs: TabInfo[]) => void>();

function notify(): void {
  for (const listener of listeners) listener(tabs);
}

export function currentTabs(): TabInfo[] {
  return tabs;
}

export function activeTab(): TabInfo | undefined {
  return tabs.find((t) => t.active);
}

export function setTabs(next: TabInfo[]): void {
  tabs = next;
  notify();
}

function merge(existing: TabInfo, update: TabInfo): TabInfo {
  if (existing.id === update.id) return update;
  return update.active ? { ...existing, active: false } : existing;
}

export function updateTab(update: TabInfo): void {
  if (!tabs.some((t) => t.id === update.id)) return;
  tabs = tabs.map((t) => merge(t, update));
  notify();
}

export function onTabs(listener: (tabs: TabInfo[]) => void): void {
  listeners.add(listener);
}
