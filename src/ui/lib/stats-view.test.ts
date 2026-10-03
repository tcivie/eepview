// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { formatPercent, formatRate, type StatsLike, statsText } from "./stats-view.ts";

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
