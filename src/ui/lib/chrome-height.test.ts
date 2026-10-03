import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  CHROME_HEIGHT,
  chromeHeight,
  FIND_BAR_HEIGHT,
  POPUP_MARGIN,
  tabStripInset,
} from "./chrome-height.ts";

describe("chromeHeight", () => {
  it("is the tab strip plus nav row when nothing is open", () => {
    assert.equal(chromeHeight({ findOpen: false, popupBottoms: [] }), CHROME_HEIGHT);
  });
  it("adds the find bar", () => {
    assert.equal(
      chromeHeight({ findOpen: true, popupBottoms: [] }),
      CHROME_HEIGHT + FIND_BAR_HEIGHT,
    );
  });
  it("grows to fit the lowest open popup", () => {
    assert.equal(chromeHeight({ findOpen: false, popupBottoms: [150.2, 300] }), 300 + POPUP_MARGIN);
  });
  it("never shrinks below the bars for a short popup", () => {
    assert.equal(
      chromeHeight({ findOpen: true, popupBottoms: [90] }),
      CHROME_HEIGHT + FIND_BAR_HEIGHT,
    );
  });
});

describe("tabStripInset", () => {
  it("reserves room for the macOS traffic lights only", () => {
    assert.equal(tabStripInset("macos"), 72);
    assert.equal(tabStripInset("windows"), 0);
    assert.equal(tabStripInset("linux"), 0);
  });
});
