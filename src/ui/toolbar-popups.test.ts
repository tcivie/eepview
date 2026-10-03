// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// Requirement tests for the toolbar side of the popups (docs/wiki/browser-shell.md, "Toolbar
// popups"). The real toolbar page runs in a DOM on a stand-in shell; the tests press, hover
// and type as a user does and read the commands and events the toolbar sends.

import assert from "node:assert/strict";
import { beforeEach, describe, it } from "node:test";
import { TOGGLE_MS } from "./lib/popup-toggle.ts";
import { type Call, sleep, stubShell, until } from "./testing/shell-stub.ts";

const stub = stubShell("toolbar.html");
const { doc } = stub;
const ITEMS = [
  { url: "http://a.i2p/", title: "A", source: "history" },
  { url: "http://b.i2p/", title: "B", source: "bookmark" },
  { url: "http://c.i2p/", title: "C", source: "history" },
];
const STATUS = { state: "ok", proxy: "127.0.0.1:4444", version: "2.0", detail: null };

let lastId = 0;
let shown: { id: number; kind: string } | null = null;

function closeShown(refocus: boolean): void {
  if (!shown) return;
  const closed = { ...shown, refocus };
  shown = null;
  queueMicrotask(() => stub.fire("popup-closed", closed));
}

stub.answer("popup_open", (args) => {
  if (shown && shown.kind !== args.kind) closeShown(false);
  lastId += 1;
  shown = { id: lastId, kind: String(args.kind) };
  return lastId;
});
stub.answer("popup_close", (args) => {
  if (shown?.id === args.id) closeShown(args.refocus === true);
  return null;
});
stub.answer("suggest", () => ITEMS);
stub.answer("tab_list", () => []);
stub.answer("router_status", () => ({ ...STATUS, managed: true, paused: false }));
await import("./toolbar.ts");

const isInput = (el: HTMLElement): el is HTMLInputElement => el.tagName === "INPUT";

function input(id: string): HTMLInputElement {
  const found = must(id);
  if (!isInput(found)) throw new Error(`#${id} is not an input`);
  return found;
}

function must(id: string): HTMLElement {
  const found = doc.getElementById(id);
  if (!found) throw new Error(`no #${id} in the toolbar page`);
  return found;
}

function fire(target: HTMLElement, types: string[], bubbles = true): void {
  for (const type of types) {
    const kind = type.startsWith("pointer") ? "PointerEvent" : "MouseEvent";
    target.dispatchEvent(stub.make(kind, type, { bubbles }));
  }
}

const press = (t: HTMLElement) => fire(t, ["pointerdown", "mousedown"]);
const release = (t: HTMLElement) => fire(t, ["pointerup", "mouseup", "click"]);
const click = (t: HTMLElement) => {
  press(t);
  release(t);
};
const hoverIn = (t: HTMLElement) => {
  fire(t, ["pointerover", "mouseover"]);
  fire(t, ["pointerenter", "mouseenter"], false);
};
const hoverOut = (t: HTMLElement) => {
  fire(t, ["pointerout", "mouseout"]);
  fire(t, ["pointerleave", "mouseleave"], false);
};

function key(target: HTMLElement, name: string): boolean {
  const event = stub.make("KeyboardEvent", "keydown", {
    key: name,
    bubbles: true,
    cancelable: true,
  });
  target.dispatchEvent(event);
  return event.defaultPrevented;
}

const opens = (kind: string): Call[] =>
  stub.commands("popup_open").filter((c) => c.args.kind === kind);
const closes = (id: number): Call[] => stub.commands("popup_close").filter((c) => c.args.id === id);
const selects = (): number[] =>
  stub.sent
    .filter((s) => s.event === "popup-select")
    .map((s) => Number(Reflect.get(s.payload as object, "index")));

async function typeInAddress(text: string): Promise<void> {
  const field = input("address");
  field.focus();
  field.value = text;
  fire(field, ["input"]);
  await until(() => opens("suggestions").length > 0);
}

beforeEach(async () => {
  closeShown(false);
  input("address").blur();
  await sleep(Math.max(TOGGLE_MS, 150) + 60);
  stub.calls.length = 0;
  stub.sent.length = 0;
});

describe("router dot: click toggles the router panel", () => {
  it("[browser-shell popups 8] a click on the dot opens the router panel under the dot", async () => {
    click(must("status"));
    assert.ok(await until(() => opens("router").length === 1), "popup_open router");
    const anchor = opens("router")[0]?.args.anchor as Record<string, unknown>;
    for (const field of ["x", "y", "width", "height"]) {
      assert.equal(typeof anchor[field], "number", `anchor.${field}`);
    }
  });

  it("[browser-shell popups 8] a second click closes the panel, by its id, and does not reopen", async () => {
    click(must("status"));
    await until(() => opens("router").length === 1);
    const id = lastId;
    await sleep(50);
    click(must("status"));
    assert.ok(await until(() => closes(id).length > 0), "popup_close with the panel's id");
    await sleep(100);
    assert.equal(opens("router").length, 1, "the second click opened a second panel");
  });
});

describe("router dot: a long press and a late click", () => {
  it("[browser-shell popups 8] a long press on the dot closes the panel and does not reopen it", async () => {
    click(must("status"));
    await until(() => opens("router").length === 1);
    const id = lastId;
    press(must("status"));
    stub.fire("popup-closed", { id, kind: "router", refocus: false });
    shown = null;
    await sleep(TOGGLE_MS + 200);
    release(must("status"));
    await sleep(150);
    assert.equal(opens("router").length, 1, "the release reopened the panel");
  });

  it("[browser-shell popups 8] a click opens the panel again once it has been closed a while", async () => {
    click(must("status"));
    await until(() => opens("router").length === 1);
    stub.fire("popup-closed", { id: lastId, kind: "router", refocus: false });
    shown = null;
    await sleep(TOGGLE_MS + 100);
    click(must("status"));
    assert.ok(await until(() => opens("router").length === 2), "a second popup_open");
  });
});

describe("router dot and menu button: focus and the menu button", () => {
  it("[browser-shell popups 8] when the panel closes with refocus, the focus goes back to the dot", async () => {
    click(must("status"));
    await until(() => opens("router").length === 1);
    stub.fire("popup-closed", { id: lastId, kind: "router", refocus: true });
    assert.ok(await until(() => doc.activeElement === must("status")));
  });

  it("[browser-shell popups 9] the menu button opens the menu and a second click closes it", async () => {
    click(must("menu-btn"));
    assert.ok(await until(() => opens("menu").length === 1), "popup_open menu");
    const id = lastId;
    await sleep(50);
    click(must("menu-btn"));
    assert.ok(await until(() => closes(id).length > 0), "popup_close with the menu's id");
    await sleep(100);
    assert.equal(opens("menu").length, 1, "the second click reopened the menu");
  });

  it("[browser-shell popups 9] a long press on the menu button closes the menu and does not reopen it", async () => {
    click(must("menu-btn"));
    await until(() => opens("menu").length === 1);
    press(must("menu-btn"));
    stub.fire("popup-closed", { id: lastId, kind: "menu", refocus: false });
    shown = null;
    await sleep(TOGGLE_MS + 200);
    release(must("menu-btn"));
    await sleep(150);
    assert.equal(opens("menu").length, 1, "the release reopened the menu");
  });

  it("[browser-shell popups 9] when the menu closes with refocus, the focus goes back to its button", async () => {
    click(must("menu-btn"));
    await until(() => opens("menu").length === 1);
    stub.fire("popup-closed", { id: lastId, kind: "menu", refocus: true });
    assert.ok(await until(() => doc.activeElement === must("menu-btn")));
  });
});

describe("router hint", () => {
  it("[browser-shell popups 7] hovering the dot shows a hint with a title and one short line", async () => {
    hoverIn(must("status"));
    assert.ok(await until(() => opens("hint").length > 0), "popup_open hint");
    const data = opens("hint")[0]?.args.data as Record<string, unknown>;
    assert.equal(typeof data.title, "string");
    assert.equal(typeof data.text, "string");
    assert.ok(String(data.title).length > 0 && String(data.text).length > 0);
    assert.ok(!String(data.text).includes("\n"), "one line");
  });

  it("[browser-shell popups 7] the hint hides, by its id, when the mouse leaves the dot", async () => {
    hoverIn(must("status"));
    await until(() => opens("hint").length > 0);
    const id = lastId;
    hoverOut(must("status"));
    assert.ok(await until(() => closes(id).length > 0), "popup_close with the hint's id");
  });

  it("[browser-shell popups 7] the hint does not show while the router panel is open", async () => {
    click(must("status"));
    await until(() => opens("router").length === 1);
    hoverOut(must("status"));
    hoverIn(must("status"));
    await sleep(800);
    assert.equal(opens("hint").length, 0, "a hint opened over the router panel");
  });
});

describe("the hint never covers the menu or the panel", () => {
  it("[browser-shell popups 6] the toolbar never asks for the hint while the menu is open", async () => {
    click(must("menu-btn"));
    await until(() => opens("menu").length === 1);
    hoverIn(must("status"));
    await sleep(800);
    assert.equal(opens("hint").length, 0, "a hint opened over the menu");
  });

  it("[browser-shell popups 6] the toolbar never asks for the hint while the router panel is open", async () => {
    click(must("status"));
    await until(() => opens("router").length === 1);
    hoverOut(must("status"));
    hoverIn(must("menu-btn"));
    hoverIn(must("status"));
    await sleep(800);
    assert.equal(opens("hint").length, 0, "a hint opened over the router panel");
  });

  it("[browser-shell popups 7] the mouse leaving the dot does not close the router panel", async () => {
    click(must("status"));
    await until(() => opens("router").length === 1);
    const id = lastId;
    hoverOut(must("status"));
    await sleep(300);
    assert.equal(closes(id).length, 0, "the panel was closed by the mouse leaving");
  });
});

describe("address suggestions", () => {
  it("[browser-shell popups 10] typing in the focused address field shows the suggestions", async () => {
    await typeInAddress("a");
    assert.equal(opens("suggestions").length > 0, true, "popup_open suggestions");
    const data = opens("suggestions")[0]?.args.data as { items: unknown[] };
    assert.equal(data.items.length, ITEMS.length);
  });

  it("[browser-shell popups 2] showing the suggestions never changes the toolbar height", async () => {
    await typeInAddress("a");
    for (const call of stub.commands("chrome_set_height")) {
      assert.ok([84, 124].includes(Number(call.args.px)), `chrome_set_height ${call.args.px}`);
    }
  });

  it("[browser-shell popups 10] the arrow keys send popup-select and open no new popup", async () => {
    await typeInAddress("a");
    const before = stub.commands("popup_open").length;
    key(must("address"), "ArrowDown");
    key(must("address"), "ArrowDown");
    assert.ok(await until(() => selects().length >= 2), "two popup-select events");
    const [first = 0, second = 0] = selects();
    assert.equal(second, first + 1, "ArrowDown moves the highlight down by one");
    key(must("address"), "ArrowUp");
    assert.ok(await until(() => selects().length >= 3));
    assert.equal(selects()[2], first, "ArrowUp moves it back");
    await sleep(100);
    assert.equal(stub.commands("popup_open").length, before, "an arrow key opened a popup");
  });
});

describe("address suggestions: keys and focus", () => {
  it("[browser-shell popups 10] Enter opens the highlighted suggestion", async () => {
    await typeInAddress("a");
    key(must("address"), "ArrowDown");
    await until(() => selects().length >= 1);
    const index = selects()[0] ?? 0;
    const prevented = key(must("address"), "Enter");
    if (!prevented) fire(must("address-form"), ["submit"]);
    assert.ok(
      await until(() => stub.commands("navigate").some((c) => c.args.input === ITEMS[index]?.url)),
      `navigate to ${ITEMS[index]?.url}`,
    );
  });
});

describe("address suggestions: Esc and focus", () => {
  it("[browser-shell popups 10] Esc closes the list, by its id", async () => {
    await typeInAddress("a");
    const id = lastId;
    key(must("address"), "Escape");
    assert.ok(await until(() => closes(id).length > 0), "popup_close with the list's id");
  });

  it("[browser-shell popups 10] leaving the address field closes the list after 150 ms", async () => {
    await typeInAddress("a");
    const id = lastId;
    const start = Date.now();
    input("address").blur();
    assert.ok(await until(() => closes(id).length > 0, 1500), "popup_close with the list's id");
    assert.ok(Date.now() - start >= 100, `closed after ${Date.now() - start} ms, too early`);
  });

  it("[browser-shell popups 10] no suggestions show no list", async () => {
    stub.answer("suggest", () => []);
    const field = input("address");
    field.focus();
    field.value = "zzz";
    fire(field, ["input"]);
    await sleep(600);
    stub.answer("suggest", () => ITEMS);
    assert.equal(opens("suggestions").length, 0, "an empty list opened a popup");
  });
});
