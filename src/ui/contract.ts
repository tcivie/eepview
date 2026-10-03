// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import type { ThemePref } from "./theme.ts";

export type TabInfo = {
  id: number;
  url: string;
  title: string;
  kind: "internal" | "web" | "console";
  loading: boolean;
  canBack: boolean;
  canForward: boolean;
  active: boolean;
  zoom: number;
  jsOn: boolean;
  bookmarked: boolean;
  icon: string | null;
};

export type NavResult = { ok: boolean; reason?: "not-i2p" | "router-down" | "invalid" };

export type Bookmark = {
  id: string;
  url: string;
  title: string;
  folder: string | null;
  created: number;
  icon: string | null;
};

export type HistoryEntry = {
  id: string;
  url: string;
  title: string;
  visited: number;
  visits: number;
  icon: string | null;
};

export type Suggestion = { url: string; title: string; source: "bookmark" | "history" };

export type RouterState = "verifying" | "ok" | "building" | "down" | "not-i2p" | "outproxy";

export type RouterStatus = {
  state: RouterState;
  proxy: string;
  version: string | null;
  detail: string | null;
  managed: boolean;
  paused: boolean;
};

export type Settings = {
  homepage: string;
  theme: ThemePref;
  jsDefault: boolean;
  history: { enabled: boolean };
  keepCookies: boolean;
  zoomDefault: number;
};

export type HistoryCursor = { visited: number; id: string };
export type HistoryQuery = { q?: string; before?: HistoryCursor; limit?: number };
export type ChromeInsets = { left: number };
export type Platform = "macos" | "windows" | "linux";
export type PopupKind = "suggestions" | "menu" | "router" | "hint";
export type PopupAnchor = { x: number; y: number; width: number; height: number };
export type SuggestionsData = { items: Suggestion[]; index: number };
export type HintData = { title: string; text: string };
export type PopupShow = {
  id: number;
  kind: PopupKind;
  anchorWidth: number;
  data: SuggestionsData | HintData | null;
};
export type PopupClosed = { id: number; kind: PopupKind; refocus: boolean };

export type BandwidthHistory = { stepSeconds: number; inBps: number[]; outBps: number[] };
export type RouterAction = "restart" | "stop";

export type RouterStats = {
  networkStatus: string | null;
  uptimeSeconds: number | null;
  routerKind: string | null;
  routerVersion: string | null;
  javaVersion: string | null;
  bandwidthInBps: number | null;
  bandwidthOutBps: number | null;
  history: BandwidthHistory | null;
  clientTunnels: number | null;
  inboundTunnels: number | null;
  outboundTunnels: number | null;
  activePeers: number | null;
  participatingTunnels: number | null;
  buildSuccessRate: number | null;
  knownRouters: number | null;
  floodfills: number | null;
};
export type ConsoleKind = "java" | "i2pd";
export type ConsoleInfo = {
  found: boolean;
  kind: ConsoleKind | null;
  origin: string | null;
  version: string | null;
};
export type ConsoleOpenResult = { ok: boolean; reason?: "no-console" | null };
export type ClearRange = "hour" | "day" | "week" | "all";
export type FindResult = { query: string; matches: number | null; active: number | null };
export type Toast = { kind: string; text: string };

export interface Commands {
  tab_new: { args: { url?: string }; result: TabInfo };
  tab_close: { args: { id: number }; result: undefined };
  tab_select: { args: { id: number }; result: undefined };
  tab_move: { args: { id: number; index: number }; result: undefined };
  tab_list: { args: Record<string, never>; result: TabInfo[] };
  navigate: { args: { input: string }; result: NavResult };
  go_back: { args: Record<string, never>; result: undefined };
  go_forward: { args: Record<string, never>; result: undefined };
  reload: { args: { hard?: boolean }; result: undefined };
  stop: { args: Record<string, never>; result: undefined };
  home: { args: Record<string, never>; result: undefined };
  find: { args: { query: string; forward: boolean; matchCase: boolean }; result: undefined };
  find_close: { args: Record<string, never>; result: undefined };
  zoom_in: { args: Record<string, never>; result: undefined };
  zoom_out: { args: Record<string, never>; result: undefined };
  zoom_reset: { args: Record<string, never>; result: undefined };
  site_js_set: { args: { host: string; on: boolean }; result: undefined };
  bookmarks_list: { args: Record<string, never>; result: Bookmark[] };
  bookmark_add: {
    args: { bookmark: { url: string; title: string; folder?: string | null } };
    result: Bookmark;
  };
  bookmark_update: { args: { bookmark: Bookmark }; result: undefined };
  bookmark_remove: { args: { id: string }; result: undefined };
  bookmark_find: { args: { url: string }; result: Bookmark | null };
  bookmarks_export: { args: Record<string, never>; result: string };
  bookmarks_export_file: { args: Record<string, never>; result: string };
  bookmarks_import: { args: { json: string }; result: number };
  history_query: { args: { query: HistoryQuery }; result: HistoryEntry[] };
  history_remove: { args: { id: string }; result: undefined };
  history_clear: { args: { range: ClearRange }; result: undefined };
  suggest: { args: { input: string }; result: Suggestion[] };
  settings_get: { args: Record<string, never>; result: Settings };
  settings_set: { args: { patch: Partial<Settings> }; result: Settings };
  router_status: { args: Record<string, never>; result: RouterStatus };
  router_stats: { args: Record<string, never>; result: RouterStats };
  router_control: { args: { action: RouterAction }; result: undefined };
  console_status: { args: Record<string, never>; result: ConsoleInfo };
  console_detect: { args: Record<string, never>; result: ConsoleInfo };
  console_open: { args: Record<string, never>; result: ConsoleOpenResult };
  connection_pause: { args: Record<string, never>; result: undefined };
  connection_resume: { args: Record<string, never>; result: undefined };
  chrome_set_height: { args: { px: number }; result: undefined };
  platform: { args: Record<string, never>; result: Platform };
  chrome_insets: { args: Record<string, never>; result: ChromeInsets };
  window_fullscreen: { args: Record<string, never>; result: boolean };
  popup_open: {
    args: { kind: PopupKind; anchor: PopupAnchor; data?: SuggestionsData | HintData };
    result: number;
  };
  popup_size: { args: { id: number; width: number; height: number }; result: undefined };
  popup_close: { args: { id: number; refocus?: boolean }; result: undefined };
}

export interface Events {
  "tabs-changed": TabInfo[];
  "tab-updated": TabInfo;
  "find-result": FindResult;
  "router-status": RouterStatus;
  "bookmarks-changed": null;
  "history-changed": null;
  "settings-changed": Settings;
  shortcut: { action: string };
  toast: Toast;
  "link-hover": { text: string; blocked: boolean };
  "fullscreen-changed": boolean;
  "chrome-insets-changed": ChromeInsets;
  "status-side": "left" | "right";
  "icons-changed": null;
  "popup-show": PopupShow;
  "popup-closed": PopupClosed;
  "popup-select": { index: number };
  "console-changed": ConsoleInfo;
}

/** Events a page sends to the shell. */
export interface UiEvents {
  "status-size": { width: number; height: number };
  "popup-select": { index: number };
}

export type CommandName = keyof Commands;
export type EventName = keyof Events;
