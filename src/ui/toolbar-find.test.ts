// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// The find bar, on the real toolbar page and a stand-in shell (docs/wiki/browser-ui.md,
// docs/wiki/ipc-contract.md).

import assert from "node:assert/strict";
import { after, beforeEach, describe, it } from "node:test";
import { sleep, until } from "./testing/shell-stub.ts";
import { bootToolbar, tab } from "./testing/toolbar-harness.ts";

const tb = await bootToolbar([tab()]);
const { stub } = tb;
const shown = (): boolean => !tb.el("findbar").hasAttribute("hidden");
const finds = () => stub.commands("find").map((c) => c.args);
const lastFind = () => finds()[finds().length - 1];

async function openBar(): Promise<void> {
  stub.fire("shortcut", { action: "open-find" });
  await until(shown);
  stub.calls.length = 0;
}

after(() => stub.window.happyDOM.close());
beforeEach(async () => {
  if (shown()) {
    tb.click(tb.el("find-close"));
    await until(() => !shown());
  }
  tb.input("find-input").value = "";
  stub.calls.length = 0;
});

describe("opening and closing the find bar", () => {
  it("[ipc-contract keyboard-shortcuts] the open-find shortcut shows the bar and focuses its field", async () => {
    stub.fire("shortcut", { action: "open-find" });
    assert.ok(await until(shown), "the bar shows");
    assert.ok(
      await until(() => stub.doc.activeElement === tb.el("find-input")),
      "the field has the focus",
    );
  });

  it("[ipc-contract window-layout] the toolbar reports 124 or more while the bar shows, and 84 when it hides", async () => {
    await openBar();
    assert.ok(
      await until(() => stub.commands("chrome_set_height").some((c) => Number(c.args.px) >= 124)),
    );
    tb.click(tb.el("find-close"));
    assert.ok(
      await until(() => stub.commands("chrome_set_height").some((c) => Number(c.args.px) === 84)),
    );
  });

  it("[ipc-contract navigation-active-tab] Esc closes the bar and sends find_close once", async () => {
    await openBar();
    tb.key(tb.el("find-input"), "Escape");
    assert.ok(await until(() => !shown()), "the bar hides");
    assert.ok(await tb.calledOnce("find_close"), "find_close once");
  });

  it("[ipc-contract navigation-active-tab] the close button closes the bar and sends find_close once", async () => {
    await openBar();
    tb.click(tb.el("find-close"));
    assert.ok(await until(() => !shown()), "the bar hides");
    assert.ok(await tb.calledOnce("find_close"), "find_close once");
  });
});

describe("searching", () => {
  it("[ipc-contract navigation-active-tab] typing searches forward without match case", async () => {
    await openBar();
    tb.type(tb.input("find-input"), "i2p");
    assert.ok(await until(() => finds().length > 0), "find");
    assert.deepEqual(lastFind(), { query: "i2p", forward: true, matchCase: false });
  });

  it("[ipc-contract navigation-active-tab] Next and Previous search the same text forward and back", async () => {
    await openBar();
    tb.type(tb.input("find-input"), "i2p");
    await until(() => finds().length > 0);
    stub.calls.length = 0;
    tb.click(tb.el("find-next"));
    assert.ok(await until(() => finds().length === 1), "next");
    assert.deepEqual(finds()[0], { query: "i2p", forward: true, matchCase: false });
    tb.click(tb.el("find-prev"));
    assert.ok(await until(() => finds().length === 2), "previous");
    assert.deepEqual(finds()[1], { query: "i2p", forward: false, matchCase: false });
  });
});

describe("searching: keys and match case", () => {
  it("[ipc-contract navigation-active-tab] Enter finds the next match and Shift+Enter the previous", async () => {
    await openBar();
    tb.type(tb.input("find-input"), "eep");
    await until(() => finds().length > 0);
    stub.calls.length = 0;
    tb.key(tb.el("find-input"), "Enter");
    assert.ok(await until(() => finds().length === 1), "Enter");
    assert.equal(finds()[0]?.forward, true);
    tb.key(tb.el("find-input"), "Enter", { shiftKey: true });
    assert.ok(await until(() => finds().length === 2), "Shift+Enter");
    assert.equal(finds()[1]?.forward, false);
  });

  it("[ipc-contract navigation-active-tab] Match case turns on and the next search uses it", async () => {
    await openBar();
    tb.type(tb.input("find-input"), "Eep");
    await until(() => finds().length > 0);
    tb.click(tb.el("find-case"));
    assert.equal(tb.el("find-case").getAttribute("aria-pressed"), "true");
    assert.ok(await until(() => lastFind()?.matchCase === true), "matchCase true");
  });
});

describe("the count", () => {
  const count = (): string => tb.el("find-count").textContent ?? "";

  it('[browser-ui 1] shows "N of M"', async () => {
    await openBar();
    stub.fire("find-result", { query: "i2p", matches: 12, active: 3 });
    assert.ok(await until(() => count() === "3 of 12"), count());
  });

  it('[browser-ui 2] shows "No matches" when the page has none', async () => {
    await openBar();
    stub.fire("find-result", { query: "zzz", matches: 0, active: null });
    assert.ok(await until(() => count() === "No matches"), count());
  });

  it('[browser-ui 3] shows "—" when the engine gives no count', async () => {
    await openBar();
    stub.fire("find-result", { query: "i2p", matches: null, active: null });
    assert.ok(await until(() => count() === "—"), count());
    await sleep(10);
  });
});
