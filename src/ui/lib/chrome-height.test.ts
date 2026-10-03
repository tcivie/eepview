// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT
import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { chromeHeight, insetPx } from "./chrome-height.ts";

describe("chromeHeight", () => {
  it("[browser-shell popups 2] is exactly 84 when the find bar is closed", () => {
    assert.equal(chromeHeight({ findOpen: false }), 84);
  });
  it("[browser-shell popups 2] is exactly 124 while the find bar is open", () => {
    assert.equal(chromeHeight({ findOpen: true }), 124);
  });
  it("[browser-shell popups 2] gives only 84 or 124 for either state", () => {
    for (const findOpen of [false, true]) {
      assert.ok([84, 124].includes(chromeHeight({ findOpen })), `findOpen=${findOpen}`);
    }
  });
  it("[browser-shell popups 2] gives the same height on every call", () => {
    assert.equal(chromeHeight({ findOpen: false }), chromeHeight({ findOpen: false }));
    assert.equal(chromeHeight({ findOpen: true }), chromeHeight({ findOpen: true }));
  });
});

describe("chrome_insets", () => {
  it("[ipc-contract window] the left inset is a length in px for the tab strip", () => {
    assert.equal(insetPx(86), "86px");
    assert.equal(insetPx(0), "0px");
  });
});
