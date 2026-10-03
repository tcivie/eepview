// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// The console tab, on the real toolbar page and a stand-in shell (the Router console wiki page,
// R23 to R27). The tab shows in the strip, the address bar shows its badge, the star and the
// JavaScript toggle are off, and back, forward and reload send their commands.

import assert from "node:assert/strict";
import { after, beforeEach, describe, it } from "node:test";
import { sleep, until } from "./testing/shell-stub.ts";
import { bootToolbar, tab } from "./testing/toolbar-harness.ts";

const CONSOLE_URL = "http://127.0.0.1:7657/home";
const CONSOLE_BADGE = `[title="The router's own console on this computer"]`;
const I2P_BADGE = `[title="Opened over I2P"]`;

const consoleTab = (over = {}) =>
  tab({
    id: 2,
    kind: "console",
    url: CONSOLE_URL,
    title: "Router console",
    zoom: 1,
    jsOn: true,
    bookmarked: false,
    icon: null,
    canBack: false,
    canForward: false,
    ...over,
  });
const webTab = (over = {}) => tab({ id: 1, active: false, ...over });

const tb = await bootToolbar([tab()]);
const { stub } = tb;

async function show(...tabs: ReturnType<typeof tab>[]): Promise<void> {
  tb.setTabs(tabs);
  await sleep(40);
}

const strip = (): HTMLElement[] => tb.all('#tabs [role="tab"]');

after(() => stub.window.happyDOM.close());
beforeEach(async () => {
  await show(tab());
  stub.calls.length = 0;
  stub.sent.length = 0;
});

describe("R23 the console tab in the strip", () => {
  it("R23: the console tab shows in the tab strip with its title", async () => {
    await show(webTab(), consoleTab({ title: "I2P Router Console - Home" }));
    assert.equal(strip().length, 2);
    assert.ok(strip()[1]?.textContent?.includes("I2P Router Console - Home"));
    assert.equal(strip()[1]?.getAttribute("aria-selected"), "true");
  });

  it("R25: until the first title arrives the tab is named Router console", async () => {
    await show(webTab(), consoleTab());
    assert.ok(strip()[1]?.textContent?.includes("Router console"));
  });

  it("R25: tab-updated changes the title of the console tab in place", async () => {
    await show(webTab(), consoleTab());
    stub.fire("tab-updated", consoleTab({ title: "Tunnels" }));
    assert.ok(await until(() => (strip()[1]?.textContent ?? "").includes("Tunnels")), "renamed");
    assert.equal(strip().length, 2);
  });
});

describe("R26 the address bar of the console tab", () => {
  it("R26: it shows the console URL and the Router console badge, not the I2P badge", async () => {
    await show(webTab(), consoleTab());
    assert.ok(tb.input("address").value.includes("127.0.0.1:7657"), tb.input("address").value);
    const badge = stub.doc.querySelector(CONSOLE_BADGE);
    assert.ok(badge, "the console badge shows");
    assert.equal(badge.textContent?.trim(), "Router console");
    assert.equal(stub.doc.querySelector(I2P_BADGE)?.hasAttribute("hidden") ?? true, true);
  });

  it("R26: a web tab shows the I2P badge and no console badge", async () => {
    await show(tab());
    assert.ok(stub.doc.querySelector(I2P_BADGE), "the I2P badge shows");
    const console = stub.doc.querySelector(CONSOLE_BADGE);
    assert.equal(console?.hasAttribute("hidden") ?? true, true);
  });

  it("R26: Enter in the address bar of a console tab sends navigate once", async () => {
    await show(webTab(), consoleTab());
    tb.type(tb.input("address"), "b.i2p");
    const prevented = tb.key(tb.input("address"), "Enter");
    if (!prevented)
      tb.input("address").form?.dispatchEvent(
        stub.make("Event", "submit", { bubbles: true, cancelable: true }),
      );
    assert.ok(await tb.calledOnce("navigate"), "navigate once");
    assert.equal(stub.commands("navigate")[0]?.args.input, "b.i2p");
  });
});

describe("R25 the star and the JavaScript toggle in a console tab", () => {
  it("R25: the bookmark star is disabled", async () => {
    await show(webTab(), consoleTab());
    assert.equal(tb.el("star").hasAttribute("disabled"), true);
    tb.click(tb.el("star"));
    await sleep(150);
    assert.equal(stub.commands("bookmark_add").length, 0);
  });

  it("R25: the JavaScript toggle is disabled", async () => {
    await show(webTab(), consoleTab());
    assert.equal(tb.el("js").hasAttribute("disabled"), true);
    tb.click(tb.el("js"));
    await sleep(150);
    assert.equal(stub.commands("site_js_set").length, 0);
  });

  it("R25: a web tab keeps both controls", async () => {
    await show(tab());
    assert.equal(tb.el("star").hasAttribute("disabled"), false);
    assert.equal(tb.el("js").hasAttribute("disabled"), false);
  });
});

describe("R27 back, forward and reload in a console tab", () => {
  it("R27: Back and Forward follow canBack and canForward of the console tab", async () => {
    await show(webTab(), consoleTab({ canBack: false, canForward: false }));
    assert.equal(tb.el("back").hasAttribute("disabled"), true);
    assert.equal(tb.el("forward").hasAttribute("disabled"), true);
    await show(webTab(), consoleTab({ canBack: true, canForward: true }));
    assert.equal(tb.el("back").hasAttribute("disabled"), false);
    assert.equal(tb.el("forward").hasAttribute("disabled"), false);
  });

  it("R27: Back sends go_back once", async () => {
    await show(webTab(), consoleTab({ canBack: true }));
    tb.click(tb.el("back"));
    assert.ok(await tb.calledOnce("go_back"), "go_back once");
  });

  it("R27: Forward sends go_forward once", async () => {
    await show(webTab(), consoleTab({ canForward: true }));
    tb.click(tb.el("forward"));
    assert.ok(await tb.calledOnce("go_forward"), "go_forward once");
  });

  it("R27: Reload sends reload once, and while the console page loads the button stops it", async () => {
    await show(webTab(), consoleTab());
    tb.click(tb.el("reload"));
    assert.ok(await tb.calledOnce("reload"), "reload once");
    stub.calls.length = 0;
    await show(webTab(), consoleTab({ loading: true }));
    tb.click(tb.el("reload"));
    assert.ok(await tb.calledOnce("stop"), "stop once");
  });
});
