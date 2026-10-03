// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// The dev backend answers the IPC contract in the browser. These tests drive it through the
// `Backend` interface, the same boundary that the pages call.

import assert from "node:assert/strict";
import { registerHooks } from "node:module";
import { describe, it } from "node:test";
import type { CommandName, HistoryEntry, NavResult, Suggestion, TabInfo } from "./contract.ts";
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
  Object.assign(globalThis, { window: { location: { search: "" } } });
  return (await import("./mock.ts")).mockBackend;
};
const backendPromise = loadBackend();
const invoke = async (cmd: CommandName, args: object): Promise<unknown> =>
  (await backendPromise).invoke(cmd, args);

const tabs = async (): Promise<TabInfo[]> => (await invoke("tab_list", {})) as TabInfo[];
const activeUrl = async (): Promise<string | undefined> =>
  (await tabs()).find((tab) => tab.active)?.url;
const navigate = async (input: string): Promise<NavResult> =>
  (await invoke("navigate", { input })) as NavResult;

describe("address bar: blank input", () => {
  for (const input of ["", "   ", "\t \n"]) {
    it(`[browser-ui 13] ignores blank input ${JSON.stringify(input)}: no navigation and no error`, async () => {
      const before = await tabs();
      const result = await navigate(input);
      assert.equal(result.reason, undefined);
      assert.deepEqual(await tabs(), before);
    });
  }
});

describe("address bar: one word", () => {
  it("[browser-ui 14] one word with no dot, colon or scheme searches history and bookmarks", async () => {
    const result = await navigate("notbob");
    assert.equal(result.ok, true);
    assert.equal(await activeUrl(), "eepview://history?q=notbob");
  });
  it("[ipc-contract navigation-active-tab] text with no dot and no scheme searches history and bookmarks", async () => {
    await navigate("reg");
    assert.equal(await activeUrl(), "eepview://history?q=reg");
  });
});

describe("address bar: not .i2p", () => {
  it("[browser-ui 15] refuses localhost:8080 as not-i2p", async () => {
    assert.deepEqual(await navigate("localhost:8080"), { ok: false, reason: "not-i2p" });
  });
  it("[browser-ui 15] refuses a clearnet host and a clearnet URL as not-i2p", async () => {
    assert.deepEqual(await navigate("example.com"), { ok: false, reason: "not-i2p" });
    assert.deepEqual(await navigate("https://example.com/a"), { ok: false, reason: "not-i2p" });
  });
  it("[ipc-contract navigation-active-tab] shows eepview://blocked?url=… for a refused address", async () => {
    await navigate("example.com");
    const shown = await activeUrl();
    assert.ok(shown?.startsWith("eepview://blocked?url="), `${shown}`);
    assert.match(shown ?? "", /example\.com/);
  });
  it("[ipc-contract navigation-active-tab] refuses lookalike hosts like stats.i2p.evil.com", async () => {
    for (const input of [
      "stats.i2p.evil.com",
      "http://stats.i2p.evil.com/",
      "http://x.i2p@evil.com",
    ]) {
      const result = await navigate(input);
      assert.deepEqual(result, { ok: false, reason: "not-i2p" }, input);
    }
  });
});

describe("address bar: unparseable", () => {
  it('[browser-ui 16] refuses input that cannot be parsed, such as "1a:b", as invalid', async () => {
    assert.deepEqual(await navigate("1a:b"), { ok: false, reason: "invalid" });
  });
});

describe("address bar: loads", () => {
  it("[ipc-contract navigation-active-tab] foo.i2p becomes http://foo.i2p/", async () => {
    assert.equal((await navigate("foo.i2p")).ok, true);
    assert.equal(await activeUrl(), "http://foo.i2p/");
  });
  it("[ipc-contract navigation-active-tab] an https URL on a .b32.i2p host loads", async () => {
    const url = "https://abc234.b32.i2p/path";
    assert.equal((await navigate(url)).ok, true);
    assert.equal(await activeUrl(), url);
  });
  it("[ipc-contract navigation-active-tab] eepview://x opens an internal page", async () => {
    assert.equal((await navigate("eepview://bookmarks")).ok, true);
    assert.equal(await activeUrl(), "eepview://bookmarks");
  });
});

describe("history", () => {
  const query = async (q: object): Promise<HistoryEntry[]> =>
    (await invoke("history_query", { query: q })) as HistoryEntry[];
  it("[ipc-contract history-1] lists newest first, by visited desc then id desc", async () => {
    const all = await query({});
    assert.ok(all.length > 2, "the dev history holds a few entries");
    for (let n = 1; n < all.length; n++) {
      const [a, b] = [all[n - 1], all[n]] as [HistoryEntry, HistoryEntry];
      assert.ok(a.visited >= b.visited, "visited never rises");
      if (a.visited === b.visited) {
        assert.ok(a.id.localeCompare(b.id, "en", { numeric: true }) > 0, "id falls on a tie");
      }
    }
  });
  it("[ipc-contract history-1] pages with the {visited, id} cursor", async () => {
    const all = await query({});
    const first = await query({ limit: 2 });
    const last = first[first.length - 1] as HistoryEntry;
    const next = await query({ before: { visited: last.visited, id: last.id }, limit: 2 });
    assert.deepEqual(first, all.slice(0, 2));
    assert.deepEqual(next, all.slice(2, 4));
  });
});

describe("suggestions", () => {
  it("[browser-ui 7] gives at most 8 suggestions", async () => {
    for (let n = 0; n < 12; n++) {
      await invoke("bookmark_add", {
        bookmark: { url: `http://many${n}.i2p/`, title: `many ${n}` },
      });
    }
    const found = (await invoke("suggest", { input: "many" })) as Suggestion[];
    assert.ok(found.length > 0, "the bookmarks match");
    assert.ok(found.length <= 8, `${found.length} suggestions`);
  });
});
