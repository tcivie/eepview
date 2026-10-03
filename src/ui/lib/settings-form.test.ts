import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  HOME_PAGE,
  homepageAddress,
  homepageMode,
  homepageValue,
  nearestZoom,
} from "./settings-form.ts";

describe("homepage setting", () => {
  it("treats the home page and an empty value as home", () => {
    assert.equal(homepageMode(HOME_PAGE), "home");
    assert.equal(homepageMode(""), "home");
    assert.equal(homepageAddress(HOME_PAGE), "");
  });
  it("shows a custom eepsite without the scheme", () => {
    assert.equal(homepageMode("http://notbob.i2p/"), "custom");
    assert.equal(homepageAddress("http://notbob.i2p/"), "notbob.i2p");
  });
  it("builds the value to save", () => {
    assert.equal(homepageValue("home", "ignored"), HOME_PAGE);
    assert.equal(homepageValue("custom", "stats.i2p"), "http://stats.i2p/");
    assert.equal(homepageValue("custom", "eepview://bookmarks"), "eepview://bookmarks");
  });
  it("refuses a clearnet homepage", () => {
    assert.equal(homepageValue("custom", "example.com"), null);
  });
});

describe("nearestZoom", () => {
  it("picks the closest offered zoom", () => {
    assert.equal(nearestZoom(1.12, [0.9, 1, 1.1, 1.25]), 1.1);
    assert.equal(nearestZoom(3, [1, 2]), 2);
  });
  it("falls back to 100% with no options", () => {
    assert.equal(nearestZoom(1.4, []), 1);
  });
});
