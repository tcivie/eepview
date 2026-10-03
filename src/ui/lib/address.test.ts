// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { eepsiteUrl, isI2pAddress } from "./address.ts";

describe("isI2pAddress", () => {
  it("accepts .i2p and .b32.i2p hosts", () => {
    assert.equal(isI2pAddress("notbob.i2p"), true);
    assert.equal(isI2pAddress("https://abc234.b32.i2p/path"), true);
  });
});

describe("eepsiteUrl", () => {
  it("adds http to a bare host", () => {
    assert.equal(eepsiteUrl("stats.i2p"), "http://stats.i2p/");
  });
  it("keeps an existing scheme", () => {
    assert.equal(eepsiteUrl("https://zzz.i2p/topics"), "https://zzz.i2p/topics");
  });
});
