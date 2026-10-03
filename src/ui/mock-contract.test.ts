// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// The dev backend stands in for the shell, so it must keep the IPC contract
// (docs/wiki/ipc-contract.md). Each test drives it through the `Backend` boundary.

import assert from "node:assert/strict";
import { registerHooks } from "node:module";
import { describe, it } from "node:test";
import type {
  Bookmark,
  CommandName,
  EventName,
  FindResult,
  HistoryEntry,
  RouterStatus,
  Settings,
  Suggestion,
  TabInfo,
} from "./contract.ts";
import type { Backend } from "./ipc.ts";

registerHooks({
  load(url, context, nextLoad) {
    if (url.endsWith(".json")) {
      return nextLoad(url, { ...context, importAttributes: { type: "json" } });
    }
    return nextLoad(url, context);
  },
});

const loadBackend = async (): Promise<Backend> => {
  const stored = new Map<string, string>();
  const localStorage = {
    getItem: (key: string): string | null => stored.get(key) ?? null,
    setItem: (key: string, value: string): void => {
      stored.set(key, value);
    },
  };
  Object.assign(globalThis, { window: { location: { search: "" } }, localStorage });
  return (await import("./mock.ts")).mockBackend;
};
const backendPromise = loadBackend();
const invoke = async (cmd: CommandName, args: object = {}): Promise<unknown> =>
  (await backendPromise).invoke(cmd, args);
const tabs = async (): Promise<TabInfo[]> => (await invoke("tab_list")) as TabInfo[];
const active = async (): Promise<TabInfo | undefined> => (await tabs()).find((t) => t.active);
const bookmarks = async (): Promise<Bookmark[]> => (await invoke("bookmarks_list")) as Bookmark[];

const nextEvent = async (event: EventName, trigger: () => Promise<unknown>): Promise<unknown> => {
  const backend = await backendPromise;
  let seen: unknown;
  const stop = await backend.listen(event, (payload) => {
    seen = payload;
  });
  await trigger();
  stop();
  return seen;
};

describe("contract: bookmarks", () => {
  it("first run seeds stats.i2p, i2p-projekt.i2p, reg.i2p and notbob.i2p", async () => {
    const urls = (await bookmarks()).map((b) => b.url).join(" ");
    for (const host of ["stats.i2p", "i2p-projekt.i2p", "reg.i2p", "notbob.i2p"]) {
      assert.ok(urls.includes(host), `${host} is seeded`);
    }
  });
  it("adds, finds, updates and removes a bookmark", async () => {
    const url = "http://contract.i2p/";
    const added = (await invoke("bookmark_add", {
      bookmark: { url, title: "Contract", folder: "Tests" },
    })) as Bookmark;
    assert.equal(added.url, url);
    assert.equal(added.folder, "Tests");
    assert.ok(added.id);
    assert.equal(((await invoke("bookmark_find", { url })) as Bookmark | null)?.title, "Contract");
    await invoke("bookmark_update", { bookmark: { ...added, title: "Renamed" } });
    assert.equal(((await invoke("bookmark_find", { url })) as Bookmark | null)?.title, "Renamed");
    await invoke("bookmark_remove", { id: added.id });
    assert.equal(await invoke("bookmark_find", { url }), null);
  });
  it("exports bookmarks as JSON and imports them back, answering the count", async () => {
    const json = (await invoke("bookmarks_export")) as string;
    assert.ok(JSON.parse(json) !== undefined);
    assert.equal(typeof (await invoke("bookmarks_import", { json })), "number");
  });
  it("suggests a bookmark by its address", async () => {
    const found = (await invoke("suggest", { input: "notbob" })) as Suggestion[];
    assert.ok(found.some((s) => s.url.includes("notbob.i2p") && s.source === "bookmark"));
  });
});

describe("contract: tabs", () => {
  it("tab_new opens eepview://home by default and the new tab becomes active", async () => {
    const tab = (await invoke("tab_new")) as TabInfo;
    assert.equal(tab.url, "eepview://home");
    assert.equal((await active())?.id, tab.id);
    assert.equal((await tabs()).filter((t) => t.active).length, 1);
  });
  it("tab_new opens the given URL", async () => {
    const tab = (await invoke("tab_new", { url: "http://reg.i2p/" })) as TabInfo;
    assert.equal(tab.url, "http://reg.i2p/");
  });
  it("tab_select makes a tab the active one", async () => {
    const first = (await tabs())[0] as TabInfo;
    await invoke("tab_select", { id: first.id });
    assert.equal((await active())?.id, first.id);
  });
  it("tab_move puts a tab at the given index", async () => {
    const all = await tabs();
    const last = all[all.length - 1] as TabInfo;
    await invoke("tab_move", { id: last.id, index: 0 });
    assert.equal((await tabs())[0]?.id, last.id);
  });
  it("tabs-changed carries the tab list after a tab opens", async () => {
    const seen = await nextEvent("tabs-changed", () => invoke("tab_new"));
    assert.deepEqual(seen, await tabs());
  });
  it("closing the last tab opens a new home tab", async () => {
    for (const tab of await tabs()) await invoke("tab_close", { id: tab.id });
    const left = await tabs();
    assert.equal(left.length, 1);
    assert.equal(left[0]?.url, "eepview://home");
  });
});

describe("contract: navigation and page controls", () => {
  it("go_back and go_forward walk the history of the active tab", async () => {
    const start = (await active())?.url;
    await invoke("navigate", { input: "foo.i2p" });
    await invoke("go_back");
    assert.equal((await active())?.url, start);
    await invoke("go_forward");
    assert.equal((await active())?.url, "http://foo.i2p/");
  });
  it("home loads the homepage", async () => {
    await invoke("navigate", { input: "foo.i2p" });
    await invoke("home");
    assert.equal((await active())?.url, "eepview://home");
  });
  it("zoom is a factor remembered on the tab, and tab-updated carries it", async () => {
    const seen = (await nextEvent("tab-updated", () => invoke("zoom_in"))) as TabInfo;
    assert.ok(seen.zoom > 1);
    await invoke("zoom_out");
    await invoke("zoom_out");
    assert.ok(((await active())?.zoom ?? 1) < 1);
    await invoke("zoom_reset");
    assert.equal((await active())?.zoom, 1);
  });
  it("site_js_set switches page JavaScript for the tab's site", async () => {
    await invoke("navigate", { input: "foo.i2p" });
    await invoke("site_js_set", { host: "foo.i2p", on: false });
    assert.equal((await active())?.jsOn, false);
    await invoke("site_js_set", { host: "foo.i2p", on: true });
    assert.equal((await active())?.jsOn, true);
  });
  it("find answers with a find-result for the query", async () => {
    const seen = (await nextEvent("find-result", () =>
      invoke("find", { query: "i2p", forward: true, matchCase: false }),
    )) as FindResult;
    assert.equal(seen.query, "i2p");
    await invoke("find_close");
  });
});

describe("contract: settings, platform and window", () => {
  it("settings_get has JavaScript on by default", async () => {
    assert.equal(((await invoke("settings_get")) as Settings).jsDefault, true);
  });
  it("settings_set merges the patch and settings-changed carries the result", async () => {
    const seen = await nextEvent("settings-changed", () =>
      invoke("settings_set", { patch: { keepCookies: true } }),
    );
    const settings = (await invoke("settings_get")) as Settings;
    assert.equal(settings.keepCookies, true);
    assert.deepEqual(seen, settings);
  });
  it("settings_set keeps a theme of system, light or dark", async () => {
    for (const theme of ["dark", "light", "system"] as const) {
      const saved = (await invoke("settings_set", { patch: { theme } })) as Settings;
      assert.equal(saved.theme, theme);
      assert.equal(((await invoke("settings_get")) as Settings).theme, theme);
    }
  });
  it("platform is macos, windows or linux", async () => {
    assert.ok(["macos", "windows", "linux"].includes((await invoke("platform")) as string));
  });
  it("chrome_insets answers the space for the window buttons", async () => {
    const insets = (await invoke("chrome_insets")) as { left: number };
    assert.ok(insets.left >= 0);
  });
  it("chrome_set_height accepts a height", async () => {
    assert.equal(await invoke("chrome_set_height", { px: 200 }), undefined);
    assert.equal(await invoke("chrome_set_height", { px: 0 }), undefined);
  });
});

describe("contract: router", () => {
  it("router_status names the state and the proxy", async () => {
    const status = (await invoke("router_status")) as RouterStatus;
    assert.ok(["verifying", "ok", "building", "down", "not-i2p"].includes(status.state));
    assert.match(status.proxy, /:\d+$/);
  });
  it("connection_pause pauses browsing and shows router-down?reason=paused", async () => {
    await invoke("connection_pause");
    assert.equal(((await invoke("router_status")) as RouterStatus).paused, true);
    assert.match((await active())?.url ?? "", /^eepview:\/\/router-down\?.*reason=paused/);
  });
  it("connection_resume ends the pause", async () => {
    await invoke("connection_resume");
    assert.equal(((await invoke("router_status")) as RouterStatus).paused, false);
  });
});

describe("contract: history", () => {
  it("history_remove drops one entry and history_clear all drops the rest", async () => {
    const all = (await invoke("history_query", { query: { limit: 10_000 } })) as HistoryEntry[];
    const [first] = all;
    assert.ok(first);
    await invoke("history_remove", { id: first.id });
    const rest = (await invoke("history_query", { query: { limit: 10_000 } })) as HistoryEntry[];
    assert.equal(rest.length, all.length - 1);
    await invoke("history_clear", { range: "all" });
    assert.deepEqual(await invoke("history_query", { query: { limit: 10_000 } }), []);
  });
});
