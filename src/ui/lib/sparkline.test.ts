// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// Requirement tests for R55 of the router console wiki page ("The chart draws the slots"):
// the chart of the Network page and the sparkline of the router panel draw the 120 slots of
// R41 across the full width. A null slot breaks the line and the area. The width is 600 and
// the height is 140. The point of value `v` in slot `i` of `n` values is
// `x = i * 600 / (n - 1)` and `y = 140 - (min(v, max) / max) * 130`, each with one decimal.

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { areaPath, linePath, scaleMax } from "./sparkline.ts";

const count = (text: string, letter: string): number => text.split(letter).length - 1;

describe("[R55] scaleMax uses the non-null values only", () => {
  it("[R55] a null slot does not change the scale", () => {
    assert.equal(scaleMax([null, 5, null, 10]), scaleMax([5, 10]));
    assert.equal(scaleMax([null, 100, null]), scaleMax([100]));
    assert.equal(scaleMax([null, null, 7, 3]), scaleMax([7, 3]));
  });

  it("[R55] the scale holds the largest non-null value", () => {
    assert.ok(scaleMax([null, 5000, null, 100]) >= 5000);
  });

  it("[R55] a series with no value has the scale of an empty series, and it is a usable divisor", () => {
    assert.equal(scaleMax([null, null, null]), scaleMax([]));
    assert.ok(Number.isFinite(scaleMax([null, null])));
    assert.ok(scaleMax([null, null]) > 0);
  });
});

describe("[R55] linePath", () => {
  it("[R55] draws a run of values as M then L, with one decimal, across the width of 600", () => {
    assert.equal(linePath([0, 5, 10], 10), "M0.0 140.0L300.0 75.0L600.0 10.0");
  });

  it("[R55] a value above the scale is held at the scale", () => {
    assert.equal(linePath([0, 20], 10), "M0.0 140.0L600.0 10.0");
  });

  it("[R55] y has one decimal", () => {
    assert.equal(linePath([0, 1], 3), "M0.0 140.0L600.0 96.7");
  });

  it("[R55] a null slot breaks the line: each run of two or more values is its own line", () => {
    assert.equal(linePath([0, 10, null, 5, 10], 10), "M0.0 140.0L150.0 10.0M450.0 75.0L600.0 10.0");
  });

  it("[R55] a value alone between two null slots draws nothing", () => {
    assert.equal(
      linePath([0, 10, null, 5, null, 5, 10], 10),
      "M0.0 140.0L100.0 10.0M500.0 75.0L600.0 10.0",
    );
  });

  it("[R55] a value alone at an end of the series draws nothing", () => {
    assert.equal(linePath([5, null, 5, 10], 10), "M400.0 75.0L600.0 10.0");
    assert.equal(linePath([5, 10, null, 5], 10), "M0.0 75.0L200.0 10.0");
  });

  it('[R55] gives "" with no run: a lone value, only nulls, or nothing', () => {
    assert.equal(linePath([5], 10), "");
    assert.equal(linePath([null, 5, null], 10), "");
    assert.equal(linePath([null, null, null], 10), "");
    assert.equal(linePath([5, null, 5, null, 5], 10), "");
    assert.equal(linePath([], 10), "");
  });
});

describe("[R55] linePath over the 120 slots", () => {
  it("[R55] draws the 120 slots across the full width: slot i at x = i * 600 / 119", () => {
    const line = linePath(
      Array.from({ length: 120 }, () => 5),
      10,
    );
    assert.ok(line.startsWith("M0.0 75.0L5.0 75.0L10.1 75.0"), line);
    assert.ok(line.includes("L302.5 75.0"), "slot 60");
    assert.ok(line.includes("L595.0 75.0"), "slot 118");
    assert.ok(line.endsWith("L600.0 75.0"), line);
    assert.equal(count(line, "M"), 1);
    assert.equal(count(line, "L"), 119);
  });

  it("[R55] with nulls in 120 slots, the x of a slot is still its slot times 600 / 119", () => {
    const values: (number | null)[] = Array.from({ length: 120 }, () => 5);
    values[10] = null;
    const line = linePath(values, 10);
    assert.equal(count(line, "M"), 2);
    assert.ok(line.includes("L45.4 75.0"), "slot 9");
    assert.ok(!line.includes("L50.4 "), "slot 10 is null, so it has no point");
    assert.ok(line.includes("M55.5 75.0"), "the second run starts at slot 11");
    assert.ok(line.endsWith("L600.0 75.0"), line);
  });
});

describe("[R55] areaPath", () => {
  it("[R55] closes the area of a run: its line, then L<x last> 140 and L<x first> 140 and Z", () => {
    assert.equal(areaPath([0, 5, 10], 10), "M0.0 140.0L300.0 75.0L600.0 10.0L600.0 140L0.0 140Z");
  });

  it("[R55] a null slot gives one closed area for each run", () => {
    assert.equal(
      areaPath([0, 10, null, 5, 10], 10),
      "M0.0 140.0L150.0 10.0L150.0 140L0.0 140ZM450.0 75.0L600.0 10.0L600.0 140L450.0 140Z",
    );
  });

  it("[R55] a value alone between two null slots draws no area", () => {
    assert.equal(
      areaPath([0, 10, null, 5, null, 5, 10], 10),
      "M0.0 140.0L100.0 10.0L100.0 140L0.0 140ZM500.0 75.0L600.0 10.0L600.0 140L500.0 140Z",
    );
  });

  it('[R55] gives "" with no run', () => {
    assert.equal(areaPath([5], 10), "");
    assert.equal(areaPath([null, 5, null], 10), "");
    assert.equal(areaPath([null, null], 10), "");
    assert.equal(areaPath([], 10), "");
  });

  it("[R55] the area of 120 slots spans the full width and has one M for each run", () => {
    const values: (number | null)[] = Array.from({ length: 120 }, () => 5);
    values[60] = null;
    const area = areaPath(values, 10);
    assert.equal(count(area, "M"), 2);
    assert.equal(count(area, "Z"), 2);
    assert.ok(area.startsWith("M0.0 75.0"), area);
    assert.ok(area.endsWith("L600.0 140L307.6 140Z"), area);
  });
});
