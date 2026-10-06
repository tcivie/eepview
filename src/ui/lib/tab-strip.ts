// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import { displayUrl, hostOf, internalPageOf } from "./address.ts";

export interface TabLike {
  id: number;
  url: string;
  title: string;
  active: boolean;
}

const INTERNAL_TITLES: Record<string, string> = {
  home: "Home",
  bookmarks: "Bookmarks",
  history: "History",
  stats: "Network",
  settings: "Settings",
  setup: "Set up",
  blocked: "Not an I2P address",
  "router-down": "Router stopped",
  "load-failed": "Page did not load",
};

export function tabTitle(tab: Pick<TabLike, "url" | "title">): string {
  if (tab.title.trim() !== "") return tab.title.trim();
  const page = internalPageOf(tab.url);
  if (page) return INTERNAL_TITLES[page] ?? page;
  return hostOf(tab.url) || displayUrl(tab.url) || "New tab";
}

export function tabMonogram(tab: Pick<TabLike, "url" | "title">): string {
  const page = internalPageOf(tab.url);
  const source = page ? "e" : hostOf(tab.url) || tabTitle(tab);
  return (source[0] ?? "?").toUpperCase();
}

export function activeIndex(tabs: TabLike[]): number {
  return tabs.findIndex((t) => t.active);
}

export function dropIndex(pointerX: number, centers: number[]): number {
  const index = centers.findIndex((center) => pointerX < center);
  return index === -1 ? centers.length : index;
}

export function moveTarget(fromIndex: number, dropAt: number): number {
  return dropAt > fromIndex ? dropAt - 1 : dropAt;
}

export function neighbour(tabs: TabLike[], currentId: number, step: number): TabLike | undefined {
  const index = tabs.findIndex((t) => t.id === currentId);
  if (index < 0 || tabs.length === 0) return undefined;
  return tabs[(index + step + tabs.length) % tabs.length];
}

export function zoomText(zoom: number): string {
  return `${Math.round(zoom * 100)}%`;
}

export type StripPart = "tab" | "empty";
export type StripAction = "close-tab" | "new-tab" | null;

const PRIMARY_BUTTON = 0;
const MIDDLE_BUTTON = 1;

/**
 * What a mouse click on the tab strip does (docs/wiki/links-and-shortcuts.md B1, B2).
 * `button` is `MouseEvent.button` and `clicks` is `MouseEvent.detail`.
 */
export function stripMouse(on: StripPart, button: number, clicks: number): StripAction {
  if (on === "tab") return button === MIDDLE_BUTTON ? "close-tab" : null;
  return button === PRIMARY_BUTTON && clicks === 2 ? "new-tab" : null;
}
