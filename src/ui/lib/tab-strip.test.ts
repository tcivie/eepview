// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  activeIndex,
  dropIndex,
  moveTarget,
  neighbour,
  tabMonogram,
  tabTitle,
  zoomText,
} from "./tab-strip.ts";

const tabs = [
  { id: 1, url: "eepview://home", title: "", active: false },
  { id: 2, url: "http://stats.i2p/", title: "Network statistics", active: true },
  { id: 3, url: "http://zzz.i2p/", title: " ", active: false },
];

describe("tab titles", () => {
  it("prefers the page title", () => {
    assert.equal(
      tabTitle({ url: "http://stats.i2p/", title: "Network statistics" }),
      "Network statistics",
    );
  });
  it("names internal pages", () => {
    assert.equal(tabTitle({ url: "eepview://home", title: "" }), "Home");
    assert.equal(tabTitle({ url: "eepview://custom", title: "" }), "custom");
  });
  it("falls back to the host", () => {
    assert.equal(tabTitle({ url: "http://zzz.i2p/x", title: " " }), "zzz.i2p");
    assert.equal(tabTitle({ url: "", title: "" }), "New tab");
  });
  it("builds a monogram for the favicon slot", () => {
    assert.equal(tabMonogram({ url: "http://zzz.i2p/", title: "" }), "Z");
    assert.equal(tabMonogram({ url: "eepview://home", title: "" }), "E");
  });
});

describe("tab order", () => {
  it("finds the active tab", () => {
    assert.equal(activeIndex(tabs), 1);
  });
  it("finds the drop slot from tab centres", () => {
    const centres = [50, 150, 250];
    assert.equal(dropIndex(10, centres), 0);
    assert.equal(dropIndex(160, centres), 2);
    assert.equal(dropIndex(400, centres), 3);
  });
  it("adjusts the target when moving right", () => {
    assert.equal(moveTarget(0, 3), 2);
    assert.equal(moveTarget(2, 0), 0);
  });
  it("cycles to the next and previous tab", () => {
    assert.equal(neighbour(tabs, 3, 1)?.id, 1);
    assert.equal(neighbour(tabs, 1, -1)?.id, 3);
    assert.equal(neighbour(tabs, 9, 1), undefined);
  });
});

describe("zoom", () => {
  it("shows the zoom factor as a percentage", () => {
    assert.equal(zoomText(1), "100%");
    assert.equal(zoomText(1.1), "110%");
  });
});
