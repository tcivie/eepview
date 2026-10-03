// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT
import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { chromeHeight, insetPx } from "./chrome-height.ts";

describe("[ipc-contract window] toolbar height", () => {
  it("[ipc-contract window] is 84 px with no find bar and no popup", () => {
    assert.equal(chromeHeight({ findOpen: false, popupBottoms: [] }), 84);
  });
  it("[ipc-contract window] is 124 px with the find bar open", () => {
    assert.equal(chromeHeight({ findOpen: true, popupBottoms: [] }), 124);
  });
  it("[ipc-contract window] grows to fit an open popup", () => {
    const height = chromeHeight({ findOpen: false, popupBottoms: [300] });
    assert.ok(height >= 300, `height ${height} must cover a popup that ends at 300`);
    assert.ok(height <= 480);
  });
  it("[ipc-contract window] grows to fit the lowest of several popups", () => {
    const height = chromeHeight({ findOpen: false, popupBottoms: [150, 320, 200] });
    assert.ok(height >= 320, `height ${height} must cover the lowest popup`);
  });
  it("[ipc-contract window] is clamped to 480 px", () => {
    assert.equal(chromeHeight({ findOpen: false, popupBottoms: [2000] }), 480);
  });
  it("[ipc-contract window] never drops below 84 px for a small popup", () => {
    assert.equal(chromeHeight({ findOpen: false, popupBottoms: [20] }), 84);
  });
});

describe("chrome_insets", () => {
  it("[ipc-contract window] the left inset is a length in px for the tab strip", () => {
    assert.equal(insetPx(86), "86px");
    assert.equal(insetPx(0), "0px");
  });
});
