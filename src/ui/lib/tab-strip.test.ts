// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { activeIndex, neighbour, stripMouse, tabTitle } from "./tab-strip.ts";

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

// docs/wiki/links-and-shortcuts.md, B1 and B2. `button` is MouseEvent.button (0 primary,
// 1 middle, 2 secondary, 3 back, 4 forward). `clicks` is MouseEvent.detail.
describe("tab strip mouse", () => {
  it("[links-and-shortcuts B1] a middle-click on a tab closes it", () => {
    assert.equal(stripMouse("tab", 1, 1), "close-tab");
  });
  it("[links-and-shortcuts B1] a middle-click on a tab closes it whatever the click count", () => {
    assert.equal(stripMouse("tab", 1, 2), "close-tab");
    assert.equal(stripMouse("tab", 1, 0), "close-tab");
  });
  it("[links-and-shortcuts B1] a middle-click on the empty strip does nothing", () => {
    assert.equal(stripMouse("empty", 1, 1), null);
    assert.equal(stripMouse("empty", 1, 2), null);
  });
  it("[links-and-shortcuts B2] a double-click on the empty strip opens a new tab", () => {
    assert.equal(stripMouse("empty", 0, 2), "new-tab");
  });
  it("[links-and-shortcuts B2] a single click on the empty strip does nothing", () => {
    assert.equal(stripMouse("empty", 0, 1), null);
  });
  it("[links-and-shortcuts B2] more than two clicks is not a double-click", () => {
    assert.equal(stripMouse("empty", 0, 3), null);
  });
  it("[links-and-shortcuts B2] a double-click on a tab does not open a new tab", () => {
    assert.equal(stripMouse("tab", 0, 2), null);
  });
  it("[links-and-shortcuts B1] a primary click on a tab does not close it", () => {
    assert.equal(stripMouse("tab", 0, 1), null);
  });
  it("[links-and-shortcuts B1 B2] the secondary, back and forward buttons do nothing", () => {
    for (const on of ["tab", "empty"] as const) {
      for (const button of [2, 3, 4]) {
        for (const clicks of [1, 2]) {
          assert.equal(stripMouse(on, button, clicks), null, `${on} button ${button} x${clicks}`);
        }
      }
    }
  });
});
