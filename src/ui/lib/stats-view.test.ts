// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  formatPercent,
  formatRate,
  formatUptime,
  type StatsLike,
  statsText,
} from "./stats-view.ts";

describe("Network page numbers", () => {
  it("[browser-ui 11] shows a rate under 1 KB in B/s with one decimal", () => {
    assert.match(formatRate(512), /^512\.0\s?B\/s$/);
  });
  it("[browser-ui 11] shows a rate in KB/s with one decimal", () => {
    assert.match(formatRate(1536), /^1\.5\s?KB\/s$/);
  });
  it("[browser-ui 11] shows a rate in MB/s with one decimal", () => {
    assert.match(formatRate(3_000_000), /^(2\.9|3\.0)\s?MB\/s$/);
  });
  it("[browser-ui 12] shows a ratio as a percentage with one decimal", () => {
    assert.equal(formatPercent(0.874), "87.4%");
    assert.equal(formatPercent(0.5), "50.0%");
    assert.equal(formatPercent(1), "100.0%");
  });
});

const STATS: StatsLike = {
  networkStatus: "OK",
  uptimeSeconds: 7380,
  routerKind: "i2pd",
  routerVersion: "2.50.0",
  javaVersion: null,
  bandwidthInBps: 1536,
  bandwidthOutBps: 512,
  history: null,
  clientTunnels: 6,
  participatingTunnels: 345,
  buildSuccessRate: 0.874,
  knownRouters: 2310,
  floodfills: 40,
};

describe("Network page", () => {
  const text = statsText(STATS);
  it("[browser-ui 11] shows the bandwidth rates with a unit and one decimal", () => {
    assert.match(text.bandwidthIn, /^1\.5\s?KB\/s$/);
    assert.match(text.bandwidthOut, /^512\.0\s?B\/s$/);
  });
  it("[browser-ui 12] shows the tunnel build success ratio as a percentage with one decimal", () => {
    assert.equal(text.buildRate, "87.4%");
  });
  it("Network page: shows the counts it was given", () => {
    assert.match(text.clientTunnels, /6/);
    assert.match(text.participatingTunnels, /345/);
    assert.match(text.knownRouters, /2310|2,310|2 310/);
  });
});

describe("[R34] uptime to its resolution", () => {
  it("[R34] with no resolution, a null one or one below 60 s, it keeps today's output", () => {
    for (const resolution of [undefined, null, 1, 30, 59]) {
      assert.equal(formatUptime(28_800, resolution), "8 h 0 min", `resolution ${resolution}`);
      assert.equal(formatUptime(7380, resolution), "2 h 3 min", `resolution ${resolution}`);
      assert.equal(formatUptime(93_784, resolution), "1 d 2 h", `resolution ${resolution}`);
      assert.equal(formatUptime(300, resolution), "5 min", `resolution ${resolution}`);
    }
    assert.equal(formatUptime(28_800), "8 h 0 min");
  });

  it('[R34] with a resolution of 3 600 s, 28 800 s is "8 h", not "8 h 0 min"', () => {
    assert.equal(formatUptime(28_800, 3600), "8 h");
  });

  it("[R34] with a resolution of 3 600 s, a day and hours drop the minutes", () => {
    assert.equal(formatUptime(7380, 3600), "2 h");
    assert.equal(formatUptime(93_784, 3600), "1 d 2 h");
  });

  it('[R34] with a resolution of 86 400 s, 172 800 s is "2 d"', () => {
    assert.equal(formatUptime(172_800, 86_400), "2 d");
    assert.equal(formatUptime(93_784, 86_400), "1 d");
  });
});

describe("[R34] uptime at a resolution of 60 s or finer than the unit", () => {
  it("[R34] with a resolution of 60 s, the minutes stay", () => {
    assert.equal(formatUptime(7380, 60), "2 h 3 min");
    assert.equal(formatUptime(28_800, 60), "8 h 0 min");
    assert.equal(formatUptime(300, 60), "5 min");
  });

  it("[R34] never shows a unit smaller than the resolution", () => {
    for (const [seconds, resolution, unit] of [
      [93_784, 3600, "min"],
      [93_784, 86_400, "h"],
      [172_800, 86_400, "min"],
      [28_800, 3600, "min"],
    ] as const) {
      const text = formatUptime(seconds, resolution);
      assert.equal(new RegExp(`\\b${unit}\\b`).test(text), false, `${text} shows ${unit}`);
    }
  });
});

describe("[R34] the Network page passes the resolution on", () => {
  it('[R34] statsText shows 28 800 s with a resolution of 3 600 s as "8 h"', () => {
    const text = statsText({ ...STATS, uptimeSeconds: 28_800, uptimeResolutionSeconds: 3600 });
    const shown = Object.values(text).filter((value) => /\b8 h\b/.test(value));
    assert.ok(shown.length > 0, `no field shows "8 h": ${JSON.stringify(text)}`);
    assert.ok(
      shown.every((value) => !value.includes("0 min")),
      `a field shows "0 min": ${JSON.stringify(shown)}`,
    );
  });

  it("[R34] statsText keeps today's uptime when the resolution is null", () => {
    const text = statsText({ ...STATS, uptimeSeconds: 28_800, uptimeResolutionSeconds: null });
    assert.ok(Object.values(text).some((value) => String(value).includes("8 h 0 min")));
  });
});
