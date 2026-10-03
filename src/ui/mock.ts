import type {
  Bookmark,
  ClearRange,
  CommandName,
  EventName,
  HistoryEntry,
  HistoryQuery,
  NavResult,
  Platform,
  RouterState,
  RouterStats,
  RouterStatus,
  Settings,
  Suggestion,
  TabInfo,
} from "./contract.ts";
import type { Backend } from "./ipc.ts";
import { isBeforeCursor, newestFirst } from "./lib/history-groups.ts";
import data from "./mock-data.json";
import { localThemeStore } from "./theme.ts";

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;
const HISTORY_PAGE = 50;
const SUGGEST_MAX = 8;
const I2P_HOST = /^(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+i2p$/;
const EEPSITE_SCHEME = "http:";
const eepsite = (site: string): string => new URL(`${EEPSITE_SCHEME}${site}`).href;
const params = new URLSearchParams(window.location.search);
const bus = new EventTarget();

function emit(event: EventName, payload: unknown): void {
  queueMicrotask(() => bus.dispatchEvent(new CustomEvent(event, { detail: payload })));
}

const now = Date.now();
let nextId = 100;
const newId = (): string => `m${nextId++}`;

const settings: Settings = {
  homepage: "eepview://home",
  theme: localThemeStore.get(),
  jsDefault: false,
  history: { enabled: true },
  keepCookies: false,
  zoomDefault: 1,
};

const bookmark = (url: string, title: string, folder: string | null, age: number): Bookmark => ({
  id: newId(),
  url,
  title,
  folder,
  created: now - age * DAY,
});

let bookmarks: Bookmark[] = data.bookmarks.map((b) =>
  bookmark(eepsite(b.site), b.title, b.folder, b.ageDays),
);

function historyEntry(i: number): HistoryEntry {
  const site = data.history[i % data.history.length] ?? { site: "stats.i2p/", title: "" };
  const url = eepsite(site.site);
  const title = site.title;
  const ageMinutes = i * 53 + (i % 3) * 7;
  return { id: `h${i}`, url, title, visited: now - ageMinutes * MINUTE, visits: 1 + (i % 4) };
}

let history: HistoryEntry[] = Array.from({ length: data.historySize }, (_, i) =>
  historyEntry(i),
).sort(newestFirst);

const tab = (id: number, url: string, title: string, extra: Partial<TabInfo> = {}): TabInfo => ({
  id,
  url,
  title,
  kind: url.startsWith("eepview:") ? "internal" : "web",
  loading: false,
  canBack: true,
  canForward: false,
  active: false,
  zoom: 1,
  jsOn: false,
  bookmarked: false,
  ...extra,
});

let tabs: TabInfo[] = data.tabs.map((t, i) => {
  const url = "site" in t && t.site ? eepsite(t.site) : (t.url ?? "eepview://home");
  return tab(i + 1, url, t.title, {
    active: t.active,
    loading: t.loading,
    bookmarked: t.bookmarked,
    canBack: i > 0,
  });
});

function routerFromParams(): RouterStatus {
  const requested = (params.get("router") ?? "ok") as RouterState;
  return {
    state: requested,
    proxy: data.proxy,
    version: data.routerVersion,
    detail: null,
    managed: params.get("managed") !== "0",
    paused: params.has("paused"),
  };
}

let router = routerFromParams();

function setRouter(patch: Partial<RouterStatus>): undefined {
  router = { ...router, ...patch };
  emit("router-status", router);
  return undefined;
}

function activeTab(): TabInfo | undefined {
  return tabs.find((t) => t.active);
}

function updateTab(id: number, patch: Partial<TabInfo>): void {
  tabs = tabs.map((t) => (t.id === id ? { ...t, ...patch } : t));
  const changed = tabs.find((t) => t.id === id);
  if (changed) emit("tab-updated", changed);
}

function selectTab(id: number): void {
  tabs = tabs.map((t) => ({ ...t, active: t.id === id }));
  emit("tabs-changed", tabs);
}

function newTab(url = "eepview://home"): TabInfo {
  const id = Math.max(0, ...tabs.map((t) => t.id)) + 1;
  const created = tab(id, url, titleFor(url), { canBack: false });
  tabs = [...tabs, created];
  selectTab(id);
  return { ...created, active: true };
}

function closeTab(id: number): void {
  const index = tabs.findIndex((t) => t.id === id);
  const wasActive = tabs[index]?.active ?? false;
  tabs = tabs.filter((t) => t.id !== id);
  if (tabs.length === 0) {
    newTab();
    return;
  }
  const next = tabs[Math.min(index, tabs.length - 1)];
  if (wasActive && next) selectTab(next.id);
  else emit("tabs-changed", tabs);
}

function moveTab(id: number, index: number): void {
  const moving = tabs.find((t) => t.id === id);
  if (!moving) return;
  const rest = tabs.filter((t) => t.id !== id);
  rest.splice(index, 0, moving);
  tabs = rest;
  emit("tabs-changed", tabs);
}

function hostOf(input: string): string {
  const bare = input
    .trim()
    .toLowerCase()
    .replace(/^https?:/, "");
  return bare.replace(/^\/+/, "").split(/[/?#]/)[0] ?? "";
}

function titleFor(url: string): string {
  if (url.startsWith("eepview://")) return url.slice("eepview://".length).split("?")[0] ?? "";
  return hostOf(url);
}

function resolveInput(input: string): { url: string; result: NavResult } {
  const trimmed = input.trim();
  if (trimmed.startsWith("eepview://")) return { url: trimmed, result: { ok: true } };
  if (I2P_HOST.test(hostOf(trimmed))) {
    const withScheme = /^https?:/.test(trimmed) ? trimmed : eepsite(trimmed);
    return { url: withScheme, result: { ok: true } };
  }
  if (!trimmed.includes(".") && !trimmed.includes(":")) {
    return { url: `eepview://history?q=${encodeURIComponent(trimmed)}`, result: { ok: true } };
  }
  const blocked = `eepview://blocked?url=${encodeURIComponent(trimmed)}`;
  return { url: blocked, result: { ok: false, reason: "not-i2p" } };
}

function navigate(input: string): NavResult {
  const { url, result } = resolveInput(input);
  const current = activeTab();
  if (current) {
    const bookmarked = bookmarks.some((b) => b.url === url);
    updateTab(current.id, { url, title: titleFor(url), kind: tab(0, url, "").kind, bookmarked });
  }
  return result;
}

function zoomBy(step: number): void {
  const current = activeTab();
  if (!current) return;
  const zoom = step === 0 ? 1 : Math.min(3, Math.max(0.3, current.zoom + step));
  updateTab(current.id, { zoom: Math.round(zoom * 10) / 10 });
}

let findIndex = 0;

function find(query: string, forward: boolean): void {
  const matches = query.length === 0 ? 0 : (query.length * 5) % 17;
  findIndex = matches === 0 ? 0 : (findIndex + (forward ? 1 : matches - 1)) % matches;
  emit("find-result", { query, matches, active: matches === 0 ? null : findIndex + 1 });
}

function matchesQuery(entry: HistoryEntry, q: string): boolean {
  const needle = q.toLowerCase();
  return entry.url.toLowerCase().includes(needle) || entry.title.toLowerCase().includes(needle);
}

function queryHistory(query: HistoryQuery): HistoryEntry[] {
  const q = query.q ?? "";
  return history
    .filter((e) => isBeforeCursor(e, query.before) && (q === "" || matchesQuery(e, q)))
    .slice(0, query.limit ?? HISTORY_PAGE);
}

const RANGE_MS: Record<ClearRange, number> = {
  hour: HOUR,
  day: DAY,
  week: 7 * DAY,
  all: Number.POSITIVE_INFINITY,
};

function clearHistory(range: ClearRange): void {
  const cutoff = Date.now() - RANGE_MS[range];
  history = history.filter((e) => e.visited < cutoff);
  emit("history-changed", null);
}

function suggest(input: string): Suggestion[] {
  const q = input.trim().toLowerCase();
  if (q === "") return [];
  const fromBookmarks = bookmarks
    .filter((b) => `${b.url} ${b.title}`.toLowerCase().includes(q))
    .map((b): Suggestion => ({ url: b.url, title: b.title, source: "bookmark" }));
  const seen = new Set(fromBookmarks.map((s) => s.url));
  const fromHistory = history
    .filter((e) => matchesQuery(e, q) && !seen.has(e.url))
    .map((e): Suggestion => ({ url: e.url, title: e.title, source: "history" }));
  const unique = [...new Map([...fromBookmarks, ...fromHistory].map((s) => [s.url, s])).values()];
  return unique.slice(0, SUGGEST_MAX);
}

function addBookmark(input: { url: string; title: string; folder?: string | null }): Bookmark {
  const created = bookmark(input.url, input.title, input.folder ?? null, 0);
  bookmarks = [...bookmarks, created];
  emit("bookmarks-changed", null);
  return created;
}

function updateBookmark(next: Bookmark): void {
  bookmarks = bookmarks.map((b) => (b.id === next.id ? next : b));
  emit("bookmarks-changed", null);
}

function removeBookmark(id: string): void {
  bookmarks = bookmarks.filter((b) => b.id !== id);
  emit("bookmarks-changed", null);
}

function importBookmarks(json: string): number {
  const parsed: unknown = JSON.parse(json);
  const list = Array.isArray(parsed) ? (parsed as Partial<Bookmark>[]) : [];
  const valid = list.filter((b) => typeof b.url === "string" && I2P_HOST.test(hostOf(b.url)));
  for (const b of valid) addBookmark({ url: b.url ?? "", title: b.title ?? "", folder: b.folder });
  return valid.length;
}

function setSettings(patch: Partial<Settings>): Settings {
  Object.assign(settings, patch);
  emit("settings-changed", { ...settings });
  return { ...settings };
}

function platformFromParams(): Platform {
  const requested = params.get("platform");
  return requested === "windows" || requested === "linux" ? requested : "macos";
}

let statsTick = 0;

function wave(i: number, base: number, swing: number): number {
  return Math.max(2048, base + Math.sin(i / 7) * swing + Math.sin(i * 1.7) * swing * 0.35);
}

function bandwidthSeries(base: number, swing: number): number[] {
  return Array.from({ length: data.stats.samples }, (_, i) => wave(i + statsTick, base, swing));
}

function sampleStats(): RouterStats {
  statsTick += 1;
  const inBps = bandwidthSeries(data.stats.inBps, data.stats.inSwing);
  const outBps = bandwidthSeries(data.stats.outBps, data.stats.outSwing);
  return {
    ...data.stats.fixed,
    uptimeSeconds: data.stats.fixed.uptimeSeconds + statsTick * 5,
    bandwidthInBps: inBps[inBps.length - 1] ?? null,
    bandwidthOutBps: outBps[outBps.length - 1] ?? null,
    history: { stepSeconds: data.stats.stepSeconds, inBps, outBps },
  };
}

type Handler = (args: Record<string, unknown>) => unknown;
const arg = <T>(args: Record<string, unknown>, key: string): T => args[key] as T;

const handlers: Record<CommandName, Handler> = {
  tab_new: (a) => newTab(arg<string | undefined>(a, "url")),
  tab_close: (a) => closeTab(arg(a, "id")),
  tab_select: (a) => selectTab(arg(a, "id")),
  tab_move: (a) => moveTab(arg(a, "id"), arg(a, "index")),
  tab_list: () => tabs,
  navigate: (a) => navigate(arg(a, "input")),
  go_back: () => undefined,
  go_forward: () => undefined,
  reload: () => undefined,
  stop: () => {
    const current = activeTab();
    if (current) updateTab(current.id, { loading: false });
  },
  home: () => navigate(settings.homepage),
  find: (a) => find(arg(a, "query"), arg(a, "forward")),
  find_close: () => undefined,
  zoom_in: () => zoomBy(0.1),
  zoom_out: () => zoomBy(-0.1),
  zoom_reset: () => zoomBy(0),
  site_js_set: (a) => {
    const current = activeTab();
    if (current) updateTab(current.id, { jsOn: arg(a, "on") });
  },
  bookmarks_list: () => bookmarks,
  bookmark_add: (a) => addBookmark(arg(a, "bookmark")),
  bookmark_update: (a) => updateBookmark(arg(a, "bookmark")),
  bookmark_remove: (a) => removeBookmark(arg(a, "id")),
  bookmark_find: (a) => bookmarks.find((b) => b.url === arg(a, "url")) ?? null,
  bookmarks_export: () => JSON.stringify({ version: 1, bookmarks }, null, 2),
  bookmarks_import: (a) => importBookmarks(arg(a, "json")),
  history_query: (a) => queryHistory(arg(a, "query")),
  history_remove: (a) => {
    history = history.filter((e) => e.id !== arg(a, "id"));
    emit("history-changed", null);
  },
  history_clear: (a) => clearHistory(arg(a, "range")),
  suggest: (a) => suggest(arg(a, "input")),
  settings_get: () => ({ ...settings }),
  settings_set: (a) => setSettings(arg(a, "patch")),
  router_status: () => router,
  router_stats: () => sampleStats(),
  router_control: (a) => setRouter({ state: arg(a, "action") === "stop" ? "down" : "building" }),
  connection_pause: () => setRouter({ paused: true }),
  connection_resume: () => setRouter({ paused: false }),
  chrome_set_height: () => undefined,
  platform: () => platformFromParams(),
  window_fullscreen: () => params.has("fullscreen"),
  chrome_insets: () => ({
    left: platformFromParams() === "macos" && !params.has("fullscreen") ? data.macInsetLeft : 0,
  }),
  bookmarks_export_file: () => data.exportPath,
};

export const mockBackend: Backend = {
  invoke: (cmd, args) => {
    try {
      return Promise.resolve(handlers[cmd](args as Record<string, unknown>));
    } catch (error) {
      return Promise.reject(error);
    }
  },
  listen: (event, handler) => {
    const listener = (e: Event) => handler((e as CustomEvent).detail);
    bus.addEventListener(event, listener);
    return Promise.resolve(() => bus.removeEventListener(event, listener));
  },
  emit: () => Promise.resolve(),
};

export function emitForReview(event: EventName, payload: unknown): void {
  emit(event, payload);
}
