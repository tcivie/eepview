import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  CHROME_HEIGHT,
  chromeHeight,
  FIND_BAR_HEIGHT,
  insetPx,
  POPUP_MARGIN,
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

describe("insetPx", () => {
  it("rounds the shell's inset to whole pixels", () => {
    assert.equal(insetPx(86), "86px");
    assert.equal(insetPx(85.6), "86px");
  });
  it("never goes below zero or accepts a broken value", () => {
    assert.equal(insetPx(-4), "0px");
    assert.equal(insetPx(Number.NaN), "0px");
  });
});
