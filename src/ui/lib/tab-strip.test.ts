// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { activeIndex, neighbour, tabTitle } from "./tab-strip.ts";

const tab = (id: number) => ({ id, url: `http://s${id}.i2p/`, title: `T${id}`, active: id === 2 });

describe("tab titles", () => {
  it("[browser-ui 10] a tab with no page title shows the host of its address", () => {
    assert.equal(tabTitle({ url: "http://notbob.i2p/path?q=1", title: "" }), "notbob.i2p");
  });
  it("[browser-ui 10] a tab with a page title shows the title", () => {
    assert.equal(tabTitle({ url: "http://notbob.i2p/", title: "NotBob" }), "NotBob");
  });
});

describe("tab switching", () => {
  const tabs = [tab(1), tab(2), tab(3)];
  it("[ipc-contract keyboard-shortcuts] Ctrl+Tab goes to the next tab", () => {
    assert.equal(neighbour(tabs, 2, 1)?.id, 3);
    assert.equal(neighbour(tabs, 1, 1)?.id, 2);
  });
  it("[ipc-contract keyboard-shortcuts] Ctrl+Shift+Tab goes to the previous tab", () => {
    assert.equal(neighbour(tabs, 2, -1)?.id, 1);
    assert.equal(neighbour(tabs, 3, -1)?.id, 2);
  });
});

describe("active tab", () => {
  it("[ipc-contract keyboard-shortcuts] the active tab is the one that TabInfo marks active", () => {
    assert.equal(activeIndex([tab(1), tab(2), tab(3)]), 1);
  });
});
