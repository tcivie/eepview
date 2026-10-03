// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  formatCount,
  formatPercent,
  formatRate,
  formatRouter,
  formatUptime,
  MISSING,
  type StatsLike,
  statsText,
} from "./stats-view.ts";

const empty: StatsLike = {
  networkStatus: null,
  uptimeSeconds: null,
  routerKind: null,
  routerVersion: null,
  javaVersion: null,
  bandwidthInBps: null,
  bandwidthOutBps: null,
  bandwidthHistory: null,
  clientTunnels: null,
  participatingTunnels: null,
  buildSuccessRate: null,
  knownRouters: null,
  floodfills: null,
};

describe("stats formatting", () => {
  it("shows a dash for every missing field", () => {
    const view = statsText(empty);
    for (const [key, value] of Object.entries(view)) {
      if (key !== "buildRateBar") assert.equal(value, MISSING, key);
    }
    assert.equal(view.buildRateBar, 0);
  });
  it("formats rates in KB/s and MB/s", () => {
    assert.equal(formatRate(49_357), "48.2 KB/s");
    assert.equal(formatRate(3 * 1024 * 1024), "3.0 MB/s");
  });
  it("formats counts, percentages and uptime", () => {
    assert.equal(formatCount(4812), "4,812");
    assert.equal(formatPercent(0.676), "68%");
    assert.equal(formatUptime(11_520), "3 h 12 min");
    assert.equal(formatUptime(300), "5 min");
    assert.equal(formatUptime(2 * 86_400 + 7200), "2 d 2 h");
  });
  it("joins the router version and kind", () => {
    assert.equal(formatRouter("bundled", "2.10.0"), "I2P 2.10.0, bundled");
    assert.equal(formatRouter(null, "2.10.0"), "I2P 2.10.0");
    assert.equal(formatRouter(null, null), MISSING);
  });
  it("clamps the build success bar", () => {
    assert.equal(statsText({ ...empty, buildSuccessRate: 1.4 }).buildRateBar, 100);
    assert.equal(statsText({ ...empty, buildSuccessRate: 0.68 }).buildRateBar, 68);
  });
});
