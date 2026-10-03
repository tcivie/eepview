// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// The toolbar buttons, on the real toolbar page and a stand-in shell (docs/wiki/browser-ui.md,
// docs/wiki/ipc-contract.md). Each button sends its command once, and the toolbar stays usable.

import assert from "node:assert/strict";
import { after, beforeEach, describe, it } from "node:test";
import { sleep, until } from "./testing/shell-stub.ts";
import { bootToolbar, tab } from "./testing/toolbar-harness.ts";

const tb = await bootToolbar([tab()]);
const { stub } = tb;

async function show(...tabs: ReturnType<typeof tab>[]): Promise<void> {
  tb.setTabs(tabs);
  await sleep(40);
}

after(() => stub.window.happyDOM.close());
beforeEach(async () => {
  await show(tab());
  stub.calls.length = 0;
  stub.sent.length = 0;
});

describe("the navigation buttons send one command", () => {
  it("[ipc-contract navigation-active-tab] Back sends go_back once", async () => {
    tb.click(tb.el("back"));
    assert.ok(await tb.calledOnce("go_back"), "go_back once");
  });

  it("[ipc-contract navigation-active-tab] Forward sends go_forward once", async () => {
    tb.click(tb.el("forward"));
    assert.ok(await tb.calledOnce("go_forward"), "go_forward once");
  });

  it("[ipc-contract navigation-active-tab] Home sends home once", async () => {
    tb.click(tb.el("home"));
    assert.ok(await tb.calledOnce("home"), "home once");
  });

  it("[ipc-contract navigation-active-tab] Reload sends reload once", async () => {
    tb.click(tb.el("reload"));
    assert.ok(await tb.calledOnce("reload"), "reload once");
    assert.equal(stub.commands("stop").length, 0);
  });

  it("[ipc-contract navigation-active-tab] while a page loads the same button stops it", async () => {
    await show(tab({ loading: true }));
    tb.click(tb.el("reload"));
    assert.ok(await tb.calledOnce("stop"), "stop once");
    assert.equal(stub.commands("reload").length, 0);
  });

  it("[ipc-contract navigation-active-tab] Back and Forward are off when the tab has no history", async () => {
    await show(tab({ canBack: false, canForward: false }));
    assert.equal(tb.el("back").hasAttribute("disabled"), true);
    assert.equal(tb.el("forward").hasAttribute("disabled"), true);
    await show(tab({ canBack: true, canForward: true }));
    assert.equal(tb.el("back").hasAttribute("disabled"), false);
    assert.equal(tb.el("forward").hasAttribute("disabled"), false);
  });
});

describe("the star and the JavaScript toggle", () => {
  it("[ipc-contract navigation-active-tab] the star bookmarks the page once", async () => {
    tb.click(tb.el("star"));
    assert.ok(await tb.calledOnce("bookmark_add"), "bookmark_add once");
    const bookmark = stub.commands("bookmark_add")[0]?.args.bookmark as { url: string };
    assert.equal(bookmark.url, "http://a.i2p/");
  });

  it("[ipc-contract navigation-active-tab] the star of a bookmarked page shows pressed and removes it", async () => {
    stub.answer("bookmark_find", () => ({
      id: "bk1",
      url: "http://a.i2p/",
      title: "A",
      folder: null,
    }));
    await show(tab({ bookmarked: true }));
    assert.equal(tb.el("star").getAttribute("aria-pressed"), "true");
    tb.click(tb.el("star"));
    assert.ok(await until(() => stub.commands("bookmark_remove").length > 0), "bookmark_remove");
    assert.equal(stub.commands("bookmark_remove")[0]?.args.id, "bk1");
    assert.equal(stub.commands("bookmark_add").length, 0);
  });
});

describe("the JavaScript toggle", () => {
  it("[ipc-contract navigation-active-tab] the JavaScript toggle turns the site's JavaScript off once", async () => {
    tb.click(tb.el("js"));
    assert.ok(await tb.calledOnce("site_js_set"), "site_js_set once");
    assert.deepEqual(stub.commands("site_js_set")[0]?.args, { host: "a.i2p", on: false });
  });

  it("[ipc-contract navigation-active-tab] the JavaScript toggle of a site with it off turns it on", async () => {
    await show(tab({ jsOn: false }));
    tb.click(tb.el("js"));
    assert.ok(await tb.calledOnce("site_js_set"), "site_js_set once");
    assert.deepEqual(stub.commands("site_js_set")[0]?.args, { host: "a.i2p", on: true });
  });

  it("[ipc-contract navigation-active-tab] the toggle shows the state of the active tab", async () => {
    const on = tb.el("js").getAttribute("aria-pressed");
    await show(tab({ jsOn: false }));
    assert.notEqual(tb.el("js").getAttribute("aria-pressed"), on);
  });
});

describe("the address bar and the toolbar events", () => {
  it("[ipc-contract navigation-active-tab] the address bar shows the address of the active tab", async () => {
    await show(tab({ url: "http://zzz.i2p/page" }));
    assert.ok(tb.input("address").value.includes("zzz.i2p"), tb.input("address").value);
  });

  it("[ipc-contract navigation-active-tab] Enter in the address bar navigates once", async () => {
    tb.type(tb.input("address"), "b.i2p");
    const prevented = tb.key(tb.input("address"), "Enter");
    if (!prevented)
      tb.input("address").form?.dispatchEvent(
        stub.make("Event", "submit", { bubbles: true, cancelable: true }),
      );
    assert.ok(await tb.calledOnce("navigate"), "navigate once");
    assert.equal(stub.commands("navigate")[0]?.args.input, "b.i2p");
  });

  it("[browser-ui 7] the address bar shows the 8 suggestions the shell gives, asked for the typed text", async () => {
    const eight = Array.from({ length: 8 }, (_, i) => ({
      url: `http://s${i}.i2p/`,
      title: `S${i}`,
      source: "history",
    }));
    stub.answer("suggest", () => eight);
    tb.type(tb.input("address"), "s");
    assert.ok(await until(() => stub.commands("popup_open").length > 0), "popup_open");
    assert.equal(stub.commands("suggest")[0]?.args.input, "s");
    const data = stub.commands("popup_open")[0]?.args.data as { items: unknown[] };
    assert.equal(data.items.length, 8);
  });
});

describe("the toolbar events", () => {
  it("[ipc-contract events] a toast shows its text", async () => {
    stub.fire("toast", { kind: "info", text: "Downloads are not supported yet." });
    assert.ok(await until(() => !tb.el("toast").hasAttribute("hidden")), "the toast shows");
    assert.equal(tb.el("toast").textContent, "Downloads are not supported yet.");
  });

  it("[ipc-contract events] the router dot follows router-status", async () => {
    stub.fire("router-status", { ...stub_status("ok") });
    await sleep(30);
    const ok = tb.el("status").getAttribute("data-tone");
    stub.fire("router-status", { ...stub_status("down") });
    await sleep(30);
    assert.notEqual(tb.el("status").getAttribute("data-tone"), ok, "the tone follows the state");
  });
});

function stub_status(state: string) {
  return {
    state,
    proxy: "127.0.0.1:4444",
    version: null,
    detail: null,
    managed: true,
    paused: false,
  };
}

describe("the shortcuts and a toolbar that stays usable", () => {
  it("[ipc-contract keyboard-shortcuts] focus-address puts the focus in the address bar", async () => {
    stub.fire("shortcut", { action: "focus-address" });
    assert.ok(await until(() => stub.doc.activeElement === tb.el("address")), "focused");
  });

  it("[ipc-contract keyboard-shortcuts] open-find and close-find show and hide the find bar", async () => {
    stub.fire("shortcut", { action: "open-find" });
    assert.ok(await until(() => !tb.el("findbar").hasAttribute("hidden")), "the find bar shows");
    stub.fire("shortcut", { action: "close-find" });
    assert.ok(await until(() => tb.el("findbar").hasAttribute("hidden")), "the find bar hides");
  });

  it("[ipc-contract navigation-active-tab] after every button the toolbar still answers", async () => {
    for (const id of ["back", "forward", "reload", "home", "star", "js"]) tb.click(tb.el(id));
    await sleep(200);
    assert.ok(stub.calls.length >= 6, `${stub.calls.length} commands for 6 buttons`);
    stub.calls.length = 0;
    tb.click(tb.el("menu-btn"));
    assert.ok(await until(() => stub.commands("popup_open").length === 1), "the menu still opens");
    tb.click(tb.el("home"));
    assert.ok(await tb.calledOnce("home"), "Home still works");
  });
});
