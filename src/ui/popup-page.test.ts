// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// Requirement tests for the popup page (docs/wiki/browser-shell.md, "Toolbar popups"). The
// real `popup` page runs in a DOM on a stand-in shell. The tests send `popup-show` and
// `popup-closed` as the shell does, then press, click and type as a user does.

import assert from "node:assert/strict";
import { after, beforeEach, describe, it } from "node:test";
import { type Call, sleep, stubShell, until } from "./testing/shell-stub.ts";

const stub = stubShell("popup.html");
const { doc } = stub;
const ITEMS = [
  { url: "http://a.i2p/", title: "A", source: "history" },
  { url: "http://b.i2p/", title: "B", source: "bookmark" },
  { url: "http://c.i2p/", title: "C", source: "history" },
];
const STATUS = { state: "ok", proxy: "127.0.0.1:4444", version: "2.0", detail: null };
// The router_stats answer in the IPC contract v1.6 shape (docs/wiki/ipc-contract.md).
const STATS = {
  version: "2.0",
  uptimeMs: 60_000,
  uptimeResolutionMs: 1,
  networkStatus: "OK",
  knownRouters: 1,
  floodfills: 1,
  activePeers: 1,
  tunnels: { in: 1, out: 1, participating: 1, client: 1, exploratory: null },
  bandwidthBytesPerSecond: { in1s: 1, out1s: 1, in5m: null, out5m: null },
  tunnelBuildSuccessPercent: { exploratory: null, client: null, total: 50 },
  history: [],
};

stub.answer("router_status", () => ({ ...STATUS, managed: true, paused: false }));
stub.answer("router_stats", () => STATS);
stub.answer("tab_list", () => []);
await import("./popup.ts");

let nextId = 100;

function must(id: string): HTMLElement {
  const found = doc.getElementById(id);
  if (!found) throw new Error(`no #${id} in the popup page`);
  return found;
}

const visible = (id: string): boolean => !must(id).hasAttribute("hidden");
const closes = (id: number): Call[] => stub.commands("popup_close").filter((c) => c.args.id === id);

function show(kind: string, data: unknown = null): number {
  nextId += 1;
  stub.fire("popup-show", { id: nextId, kind, anchorWidth: 400, data });
  return nextId;
}

function key(name: string): void {
  doc.dispatchEvent(stub.make("KeyboardEvent", "keydown", { key: name, bubbles: true }));
}

function mouse(target: Element, type: string): void {
  target.dispatchEvent(stub.make("MouseEvent", type, { bubbles: true }));
}

function need(selector: string): Element {
  const found = doc.querySelector(selector);
  if (!found) throw new Error(`nothing matches ${selector}`);
  return found;
}

const options = (): Element[] => [...doc.querySelectorAll('[role="option"]')];

// The router panel polls on a timer; closing the window stops it so the process can end.
after(() => stub.window.happyDOM.close());

beforeEach(() => {
  stub.fire("popup-closed", { id: nextId, kind: "menu", refocus: false });
  stub.calls.length = 0;
  stub.sent.length = 0;
});

describe("one card at a time", () => {
  it("[browser-shell popups 6] the suggestions card shows the items and no other card shows", async () => {
    show("suggestions", { items: ITEMS, index: 0 });
    assert.ok(await until(() => options().length === ITEMS.length), "three options");
    assert.equal(visible("suggestions"), true);
    for (const other of ["hint", "menu", "router-panel"]) assert.equal(visible(other), false);
  });

  it("[browser-shell popups 6] a popup of another kind replaces the card", async () => {
    show("suggestions", { items: ITEMS, index: 0 });
    await until(() => visible("suggestions"));
    show("menu");
    assert.ok(await until(() => visible("menu")), "the menu shows");
    assert.equal(visible("suggestions"), false);
  });

  it("[browser-shell popups 12] a popup-closed for the shown popup hides its card", async () => {
    const id = show("menu");
    await until(() => visible("menu"));
    stub.fire("popup-closed", { id, kind: "menu", refocus: false });
    assert.ok(await until(() => !visible("menu")), "the card is hidden");
  });

  it("[browser-shell popups 12] a popup-closed for another id leaves the card", async () => {
    const id = show("menu");
    await until(() => visible("menu"));
    stub.fire("popup-closed", { id: id - 1, kind: "menu", refocus: false });
    await sleep(100);
    assert.equal(visible("menu"), true, "a stale popup-closed hid the card");
  });
});

describe("suggestions and the hint", () => {
  it("[browser-shell popups 10] a pick acts on mousedown, not on click", async () => {
    show("suggestions", { items: ITEMS, index: 0 });
    await until(() => options().length === ITEMS.length);
    mouse(options()[1] ?? need("#none"), "mousedown");
    assert.ok(
      await until(() => stub.commands("navigate").some((c) => c.args.input === ITEMS[1]?.url)),
      "navigate to the picked suggestion on mousedown",
    );
  });

  it("[browser-shell popups 10] popup-select moves the highlight, and sends no new size", async () => {
    show("suggestions", { items: ITEMS, index: 0 });
    await until(() => options().length === ITEMS.length);
    await sleep(100);
    const sizes = stub.commands("popup_size").length;
    stub.fire("popup-select", { index: 2 });
    const marked = (): string[] => options().map((o) => o.getAttribute("aria-selected") ?? "");
    assert.ok(await until(() => marked()[2] === "true"), "the third option is highlighted");
    assert.deepEqual(marked(), ["false", "false", "true"]);
    await sleep(100);
    assert.equal(stub.commands("popup_size").length, sizes, "an arrow key re-measured the popup");
  });

  it("[browser-shell popups 7] the hint shows its title and its line and takes no focus", async () => {
    show("hint", { title: "Router is up", text: "Connected to I2P." });
    assert.ok(await until(() => visible("hint")), "the hint card shows");
    assert.equal(must("hint-title").textContent, "Router is up");
    assert.equal(must("hint-text").textContent, "Connected to I2P.");
    assert.equal(must("hint").contains(doc.activeElement), false);
  });
});

describe("the menu and the router panel", () => {
  it("[browser-shell popups 8] Esc closes the router panel by its id and gives the focus back", async () => {
    const id = show("router");
    await until(() => visible("router-panel"));
    key("Escape");
    assert.ok(await until(() => closes(id).length > 0), "popup_close with the panel's id");
    assert.equal(closes(id)[0]?.args.refocus, true, "refocus: true");
  });

  it("[browser-shell popups 9] Esc closes the menu by its id and gives the focus back", async () => {
    const id = show("menu");
    await until(() => visible("menu"));
    key("Escape");
    assert.ok(await until(() => closes(id).length > 0), "popup_close with the menu's id");
    assert.equal(closes(id)[0]?.args.refocus, true, "refocus: true");
  });

  it("[browser-shell popups 8] a popup that has no popup open sends no close on Esc", async () => {
    const id = show("menu");
    await until(() => visible("menu"));
    stub.fire("popup-closed", { id, kind: "menu", refocus: false });
    await until(() => !visible("menu"));
    key("Escape");
    await sleep(150);
    assert.equal(stub.commands("popup_close").length, 0, "closed a popup that was not open");
  });

  it("[browser-shell popups 8] the panel closes, by its id, when the popup loses the focus", async () => {
    // A click on the toolbar or on the page moves the focus away from the popup webview.
    const id = show("router");
    await until(() => visible("router-panel"));
    stub.dispatchOnWindow(stub.make("Event", "blur"));
    assert.ok(await until(() => closes(id).length > 0), "popup_close with the panel's id");
  });
});

describe("the menu items", () => {
  it("[browser-shell popups 9] a menu item that opens a page closes the menu", async () => {
    const id = show("menu");
    await until(() => visible("menu"));
    const item = need('[data-open="eepview://history"]');
    mouse(item, "click");
    assert.ok(await until(() => closes(id).length > 0), "popup_close with the menu's id");
    const opened = stub.calls.filter((c) => c.cmd === "navigate" || c.cmd === "tab_new");
    assert.ok(opened.length > 0, "the page opens");
  });

  it("[browser-shell popups 9] the new-tab item opens a tab and closes the menu", async () => {
    const id = show("menu");
    await until(() => visible("menu"));
    mouse(need('[data-command="new-tab"]'), "click");
    assert.ok(await until(() => stub.commands("tab_new").length > 0), "tab_new");
    assert.ok(await until(() => closes(id).length > 0), "popup_close with the menu's id");
  });

  it("[browser-shell popups 9] the zoom buttons keep the menu open", async () => {
    const id = show("menu");
    await until(() => visible("menu"));
    for (const command of ["zoom-in", "zoom-out", "zoom-reset"]) {
      mouse(need(`[data-command="${command}"]`), "click");
    }
    assert.ok(await until(() => stub.commands("zoom_reset").length > 0), "zoom_reset");
    assert.equal(stub.commands("zoom_in").length, 1);
    assert.equal(stub.commands("zoom_out").length, 1);
    await sleep(100);
    assert.equal(closes(id).length, 0, "a zoom button closed the menu");
    assert.equal(visible("menu"), true);
  });
});

describe("the page reports its size", () => {
  it("[browser-shell popups 15] the page reports the size of each popup it shows, by its id", async () => {
    for (const kind of ["menu", "router"]) {
      stub.calls.length = 0;
      const id = show(kind);
      const sized = (): boolean => stub.commands("popup_size").some((c) => c.args.id === id);
      assert.ok(await until(sized), `popup_size for ${kind} ${id}`);
    }
  });
});
