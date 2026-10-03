// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { findCountText, findTone } from "./find-text.ts";

describe("find count text", () => {
  it("is empty with no query or no result", () => {
    assert.equal(findCountText(null), "");
    assert.equal(findCountText({ query: "", matches: 0, active: null }), "");
    assert.equal(findTone(null), "idle");
  });
  it("shows the position when the engine counts", () => {
    assert.equal(findCountText({ query: "i2p", matches: 12, active: 3 }), "3 of 12");
    assert.equal(findCountText({ query: "i2p", matches: 4, active: null }), "1 of 4");
    assert.equal(findTone({ query: "i2p", matches: 4, active: 1 }), "found");
  });
  it("says not found for zero matches", () => {
    assert.equal(findCountText({ query: "zzz", matches: 0, active: null }), "Not found");
    assert.equal(findTone({ query: "zzz", matches: 0, active: null }), "missing");
  });
  it("falls back to found or not found without a count", () => {
    assert.equal(findCountText({ query: "a", matches: null, active: 1 }), "Found");
    assert.equal(findCountText({ query: "a", matches: null, active: null }), "Not found");
  });
});
