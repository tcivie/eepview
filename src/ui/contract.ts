import type { ThemePref } from "./theme.ts";

export type TabInfo = {
  id: number;
  url: string;
  title: string;
  kind: "internal" | "web";
  loading: boolean;
  canBack: boolean;
  canForward: boolean;
  active: boolean;
  zoom: number;
  jsOn: boolean;
  bookmarked: boolean;
};

export type NavResult = { ok: boolean; reason?: "not-i2p" | "router-down" | "invalid" };

export type Bookmark = {
  id: string;
  url: string;
  title: string;
  folder: string | null;
  created: number;
};

export type HistoryEntry = {
  id: string;
  url: string;
  title: string;
  visited: number;
  visits: number;
};

export type Suggestion = { url: string; title: string; source: "bookmark" | "history" };

export type RouterState = "verifying" | "ok" | "building" | "down" | "not-i2p" | "outproxy";

export type RouterStatus = {
  state: RouterState;
  proxy: string;
  version: string | null;
  detail: string | null;
};

export type Settings = {
  homepage: string;
  theme: ThemePref;
  jsDefault: boolean;
  history: { enabled: boolean };
  keepCookies: boolean;
  zoomDefault: number;
};

export type HistoryQuery = { q?: string; before?: number; limit?: number };
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
  bookmarks_import: { args: { json: string }; result: number };
  history_query: { args: { query: HistoryQuery }; result: HistoryEntry[] };
  history_remove: { args: { id: string }; result: undefined };
  history_clear: { args: { range: ClearRange }; result: undefined };
  suggest: { args: { input: string }; result: Suggestion[] };
  settings_get: { args: Record<string, never>; result: Settings };
  settings_set: { args: { patch: Partial<Settings> }; result: Settings };
  router_status: { args: Record<string, never>; result: RouterStatus };
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
}

export type CommandName = keyof Commands;
export type EventName = keyof Events;
