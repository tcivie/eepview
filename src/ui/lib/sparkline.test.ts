// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { areaPath, linePath, scaleMax } from "./sparkline.ts";

describe("sparkline", () => {
  it("rounds the scale up to 1, 2 or 5 times a power of ten", () => {
    assert.equal(scaleMax([47_000, 31_000]), 50_000);
    assert.equal(scaleMax([120]), 200);
    assert.equal(scaleMax([900]), 1000);
    assert.equal(scaleMax([]), 1);
  });
  it("draws a line from left to right", () => {
    assert.equal(linePath([0, 100], 100), "M0.0 140.0L600.0 10.0");
  });
  it("closes the area along the base", () => {
    assert.equal(areaPath([0, 100], 100), "M0.0 140.0L600.0 10.0L600 140L0 140Z");
  });
  it("clips values above the scale", () => {
    assert.equal(linePath([200], 100), "M0.0 10.0");
  });
});
