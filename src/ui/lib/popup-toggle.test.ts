// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { clickOpens, TOGGLE_MS } from "./popup-toggle.ts";

const NOW = 10_000;

describe("clickOpens: the toggle window", () => {
  it("[browser-shell popups 8] is 300 ms", () => {
    assert.equal(TOGGLE_MS, 300);
  });
  it("[browser-shell popups 8] opens the router panel when nothing is open and nothing closed", () => {
    assert.equal(clickOpens(null, undefined, "router", NOW), true);
  });
  it("[browser-shell popups 8] does not open it again while it is open", () => {
    assert.equal(clickOpens("router", undefined, "router", NOW), false);
  });
  it("[browser-shell popups 8] a second click on the dot right after it closed does not reopen", () => {
    assert.equal(clickOpens(null, NOW, "router", NOW), false);
    assert.equal(clickOpens(null, NOW - 100, "router", NOW), false);
  });
  it("[browser-shell popups 8] does not reopen 1 ms before the window ends", () => {
    assert.equal(clickOpens(null, NOW - (TOGGLE_MS - 1), "router", NOW), false);
  });
  it("[browser-shell popups 8] opens again when it closed 300 ms ago or longer", () => {
    assert.equal(clickOpens(null, NOW - TOGGLE_MS, "router", NOW), true);
    assert.equal(clickOpens(null, NOW - 5000, "router", NOW), true);
  });
});

describe("clickOpens: the menu and the other kinds", () => {
  it("[browser-shell popups 9] acts the same for the menu button", () => {
    assert.equal(clickOpens(null, undefined, "menu", NOW), true);
    assert.equal(clickOpens("menu", undefined, "menu", NOW), false);
    assert.equal(clickOpens(null, NOW - 100, "menu", NOW), false);
    assert.equal(clickOpens(null, NOW - TOGGLE_MS, "menu", NOW), true);
  });
  it("[browser-shell popups 6] opens the menu while the router panel is open", () => {
    assert.equal(clickOpens("router", undefined, "menu", NOW), true);
  });
  it("[browser-shell popups 6] opens the router panel while the menu is open", () => {
    assert.equal(clickOpens("menu", undefined, "router", NOW), true);
  });
  it("[browser-shell popups 6] opens a button while the suggestions are open", () => {
    assert.equal(clickOpens("suggestions", undefined, "menu", NOW), true);
    assert.equal(clickOpens("suggestions", undefined, "router", NOW), true);
  });
  it("[browser-shell popups 8] a recent close of this kind blocks it even when another is open", () => {
    assert.equal(clickOpens("menu", NOW - 100, "router", NOW), false);
  });
});
