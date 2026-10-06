// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT
import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { shortAddress } from "./address.ts";

describe("shortAddress", () => {
  it("[browser-shell F7] keeps a short address as it is", () => {
    assert.equal(shortAddress("http://a.i2p/"), "http://a.i2p/");
  });
  it("[browser-shell F7] keeps an empty text empty", () => {
    assert.equal(shortAddress(""), "");
  });
  it("[browser-shell F7] keeps a text of exactly 2048 characters", () => {
    const text = "a".repeat(2048);
    assert.equal(shortAddress(text), text);
  });
  it("[browser-shell F7] cuts a text of 2049 characters to 2047 plus an ellipsis", () => {
    const text = "a".repeat(2049);
    const shown = shortAddress(text);
    assert.equal(shown.length, 2048);
    assert.equal(shown, `${"a".repeat(2047)}…`);
  });
  it("[browser-shell F7] cuts a much longer text to at most 2048 characters", () => {
    const text = `http://a.i2p/${"z".repeat(10000)}`;
    const shown = shortAddress(text);
    assert.ok(shown.length <= 2048);
    assert.ok(shown.endsWith("…"));
    assert.equal(shown, `${text.slice(0, 2047)}…`);
  });
  it("[browser-shell F7] never gives more than 2048 characters", () => {
    for (const length of [0, 1, 2047, 2048, 2049, 4096]) {
      assert.ok(shortAddress("x".repeat(length)).length <= 2048, `length ${length}`);
    }
  });
});
