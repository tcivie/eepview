// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// The tab strip, on the real toolbar page and a stand-in shell (docs/wiki/browser-ui.md,
// docs/wiki/ipc-contract.md). Each action sends its command once.

import assert from "node:assert/strict";
import { after, beforeEach, describe, it } from "node:test";
import { sleep, until } from "./testing/shell-stub.ts";
import { bootToolbar, tab } from "./testing/toolbar-harness.ts";

const THREE = [
  tab({ id: 1, url: "http://a.i2p/", title: "Alpha", active: true }),
  tab({ id: 2, url: "http://b.i2p/x", title: "", active: false }),
  tab({ id: 3, url: "http://c.i2p/", title: "Gamma", active: false }),
];
const tb = await bootToolbar(THREE);
const { stub } = tb;
const tabs = (): HTMLElement[] => tb.all('#tabs [role="tab"]');

after(() => stub.window.happyDOM.close());
beforeEach(async () => {
  tb.setTabs(THREE);
  await until(() => tabs().length === 3);
  await sleep(30);
  stub.calls.length = 0;
});

describe("the tab strip shows the tabs", () => {
  it("[ipc-contract events] shows one tab for each tab of tabs-changed, the active one selected", () => {
    assert.equal(tabs().length, 3);
    assert.deepEqual(
      tabs().map((t) => t.getAttribute("aria-selected")),
      ["true", "false", "false"],
    );
  });

  it("[browser-ui 10] a tab with a title shows the title", () => {
    assert.ok(tabs()[0]?.textContent?.includes("Alpha"));
  });

  it("[browser-ui 10] a tab with no page title shows the host of its address", () => {
    const text = tabs()[1]?.textContent ?? "";
    assert.ok(text.includes("b.i2p"), text);
    assert.ok(!text.includes("/x"), "only the host");
  });

  it("[ipc-contract events] tabs-changed with fewer tabs removes the others", async () => {
    tb.setTabs([THREE[0] ?? tab()]);
    assert.ok(await until(() => tabs().length === 1), "one tab");
  });

  it("[ipc-contract events] tab-updated changes the title of that tab in place", async () => {
    stub.fire("tab-updated", { ...THREE[2], title: "Renamed" });
    assert.ok(await until(() => (tabs()[2]?.textContent ?? "").includes("Renamed")), "renamed");
    assert.equal(tabs().length, 3);
  });
});

describe("the tab strip sends one command for each action", () => {
  it("[ipc-contract navigation-active-tab] the new tab button sends tab_new once", async () => {
    tb.click(tb.el("new-tab"));
    assert.ok(await tb.calledOnce("tab_new"), "tab_new once");
  });

  it("[ipc-contract navigation-active-tab] a click on another tab selects it, once", async () => {
    const second = tabs()[2];
    if (!second) throw new Error("no third tab");
    tb.click(second);
    assert.ok(await tb.calledOnce("tab_select"), "tab_select once");
    assert.equal(stub.commands("tab_select")[0]?.args.id, 3);
  });

  it("[ipc-contract navigation-active-tab] the close button closes that tab, once", async () => {
    const button = tabs()[1]?.querySelector<HTMLElement>(".tab-close");
    if (!button) throw new Error("no close button");
    tb.click(button);
    assert.ok(await tb.calledOnce("tab_close"), "tab_close once");
    assert.equal(stub.commands("tab_close")[0]?.args.id, 2);
  });
});

describe("the tab strip: middle click, drag and a strip that stays usable", () => {
  it("[ipc-contract navigation-active-tab] a middle click closes the tab, once", async () => {
    const target = tabs()[2];
    if (!target) throw new Error("no third tab");
    for (const type of ["mousedown", "mouseup", "auxclick"]) {
      target.dispatchEvent(stub.make("MouseEvent", type, { bubbles: true, button: 1 }));
    }
    assert.ok(await tb.calledOnce("tab_close"), "tab_close once");
    assert.equal(stub.commands("tab_close")[0]?.args.id, 3);
  });

  it("[ipc-contract navigation-active-tab] dragging a tab onto another moves it there", async () => {
    const [first, , third] = tabs();
    if (!first || !third) throw new Error("no tabs to drag");
    const data = new stub.window.DataTransfer();
    for (const [target, type] of [
      [first, "dragstart"],
      [third, "dragenter"],
      [third, "dragover"],
      [third, "drop"],
      [first, "dragend"],
    ] as const) {
      target.dispatchEvent(
        stub.make("DragEvent", type, { bubbles: true, cancelable: true, dataTransfer: data }),
      );
    }
    assert.ok(await tb.calledOnce("tab_move"), "tab_move once");
    assert.deepEqual(stub.commands("tab_move")[0]?.args, { id: 1, index: 2 });
  });

  it("[ipc-contract navigation-active-tab] the strip keeps working after every action", async () => {
    tb.click(tb.el("new-tab"));
    const target = tabs()[2];
    if (target) tb.click(target);
    await sleep(200);
    stub.calls.length = 0;
    tb.click(tb.el("new-tab"));
    assert.ok(await tb.calledOnce("tab_new"), "the new tab button still works");
  });
});
