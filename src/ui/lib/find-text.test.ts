// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT
import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { findCountText } from "./find-text.ts";

describe("find bar count", () => {
  it('[browser-ui 1] shows the active match and the count as "N of M"', () => {
    assert.equal(findCountText({ query: "i2p", matches: 12, active: 3 }), "3 of 12");
    assert.equal(findCountText({ query: "i2p", matches: 1, active: 1 }), "1 of 1");
  });
  it('[browser-ui 2] shows "No matches" when the page has no match', () => {
    assert.equal(findCountText({ query: "zzz", matches: 0, active: null }), "No matches");
    assert.equal(findCountText({ query: "zzz", matches: 0, active: 0 }), "No matches");
  });
  it('[browser-ui 3] shows "—" when the engine gives no count', () => {
    assert.equal(findCountText({ query: "i2p", matches: null, active: null }), "—");
  });
});
