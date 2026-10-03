// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// Requirement tests for R33 of docs/wiki/router-console.md ("The UI reads the contract
// shape"): `statsView` turns the `RouterStats` of the IPC contract v1.6 into the view that
// the router panel and the Network page show.

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { RouterStats } from "../contract.ts";
import { HISTORY_STEP_SECONDS, type StatsView, statsView } from "./router-stats.ts";

const NULL_VIEW: StatsView = {
  networkStatus: null,
  uptimeSeconds: null,
  uptimeResolutionSeconds: null,
  routerKind: null,
  routerVersion: null,
  javaVersion: null,
  bandwidthInBps: null,
  bandwidthOutBps: null,
  history: null,
  clientTunnels: null,
  exploratoryTunnels: null,
  inboundTunnels: null,
  outboundTunnels: null,
  activePeers: null,
  participatingTunnels: null,
  buildSuccessRate: null,
  knownRouters: null,
  floodfills: null,
};

/** A contract v1.6 answer with every field null and no history. */
const emptyStats = (): RouterStats => ({
  version: null,
  uptimeMs: null,
  uptimeResolutionMs: null,
  networkStatus: null,
  knownRouters: null,
  floodfills: null,
  activePeers: null,
  tunnels: { in: null, out: null, participating: null, client: null, exploratory: null },
  bandwidthBytesPerSecond: { in1s: null, out1s: null, in5m: null, out5m: null },
  tunnelBuildSuccessPercent: { exploratory: null, client: null, total: null },
  history: [],
});

const stats = (patch: Partial<RouterStats> = {}): RouterStats => ({ ...emptyStats(), ...patch });

const FULL: RouterStats = {
  version: "2.58.0",
  uptimeMs: 93_784_000,
  uptimeResolutionMs: 1000,
  networkStatus: "OK",
  knownRouters: 3021,
  floodfills: 812,
  activePeers: 55,
  tunnels: { in: 3, out: 4, participating: 157, client: 14, exploratory: 9 },
  bandwidthBytesPerSecond: { in1s: 12_636, out1s: 5806, in5m: 11_000, out5m: 5000 },
  tunnelBuildSuccessPercent: { exploratory: 70, client: 80, total: 42 },
  history: [
    { t: 1000, in: 10, out: 20 },
    { t: 6000, in: 30, out: 40 },
    { t: 11_000, in: 50, out: 60 },
  ],
};

const FULL_VIEW: StatsView = {
  networkStatus: "OK",
  uptimeSeconds: 93_784,
  uptimeResolutionSeconds: 1,
  routerKind: null,
  routerVersion: "2.58.0",
  javaVersion: null,
  bandwidthInBps: 12_636,
  bandwidthOutBps: 5806,
  history: { stepSeconds: 5, inBps: [10, 30, 50], outBps: [20, 40, 60] },
  clientTunnels: 14,
  exploratoryTunnels: 9,
  inboundTunnels: 3,
  outboundTunnels: 4,
  activePeers: 55,
  participatingTunnels: 157,
  buildSuccessRate: 0.42,
  knownRouters: 3021,
  floodfills: 812,
};

describe("[R33] statsView maps the contract shape to the view", () => {
  it("[R33] gives every view field from its RouterStats field", () => {
    assert.deepEqual(statsView(FULL), FULL_VIEW);
  });

  it("[R33] shows the shortest bandwidth window, in1s and out1s, not the 5 minute one", () => {
    const view = statsView(FULL);
    assert.equal(view.bandwidthInBps, 12_636);
    assert.equal(view.bandwidthOutBps, 5806);
  });

  it("[R33] gives the routerKind and the javaVersion as null", () => {
    const view = statsView(FULL);
    assert.equal(view.routerKind, null);
    assert.equal(view.javaVersion, null);
  });

  it("[R33] the history step is 5 seconds", () => {
    assert.equal(HISTORY_STEP_SECONDS, 5);
    assert.equal(statsView(FULL).history?.stepSeconds, 5);
  });
});

describe("[R33] statsView with a missing input", () => {
  it("[R33] a null input gives a view with every field null", () => {
    assert.deepEqual(statsView(null), NULL_VIEW);
  });

  it("[R33] an undefined input gives a view with every field null", () => {
    assert.deepEqual(statsView(undefined), NULL_VIEW);
  });

  it("[R33] an answer with every field null gives a view with every field null", () => {
    assert.deepEqual(statsView(emptyStats()), NULL_VIEW);
  });
});

describe("[R33] statsView uptime", () => {
  it("[R33] uptimeSeconds is floor(uptimeMs / 1000)", () => {
    assert.equal(statsView(stats({ uptimeMs: 28_800_999 })).uptimeSeconds, 28_800);
    assert.equal(statsView(stats({ uptimeMs: 999 })).uptimeSeconds, 0);
    assert.equal(statsView(stats({ uptimeMs: 0 })).uptimeSeconds, 0);
    assert.equal(statsView(stats({ uptimeMs: null })).uptimeSeconds, null);
  });

  it("[R33] uptimeResolutionSeconds is max(1, floor(uptimeResolutionMs / 1000))", () => {
    const resolution = (ms: number | null): number | null =>
      statsView(stats({ uptimeResolutionMs: ms })).uptimeResolutionSeconds;
    assert.equal(resolution(3_600_000), 3600);
    assert.equal(resolution(86_400_000), 86_400);
    assert.equal(resolution(60_000), 60);
    assert.equal(resolution(2500), 2);
    assert.equal(resolution(1000), 1);
  });

  it("[R33] a resolution under one second is at least 1 second", () => {
    const resolution = (ms: number): number | null =>
      statsView(stats({ uptimeResolutionMs: ms })).uptimeResolutionSeconds;
    assert.equal(resolution(1), 1);
    assert.equal(resolution(999), 1);
    assert.equal(resolution(0), 1);
  });

  it("[R33] a null uptimeResolutionMs gives a null uptimeResolutionSeconds", () => {
    assert.equal(statsView(stats({ uptimeResolutionMs: null })).uptimeResolutionSeconds, null);
  });
});

describe("[R33] statsView build success", () => {
  const rate = (build: RouterStats["tunnelBuildSuccessPercent"]): number | null =>
    statsView(stats({ tunnelBuildSuccessPercent: build })).buildSuccessRate;

  it("[R33] is total / 100 when total is set", () => {
    assert.equal(rate({ exploratory: 10, client: 20, total: 42 }), 0.42);
    assert.equal(rate({ exploratory: null, client: null, total: 100 }), 1);
  });

  it("[R33] is exploratory / 100 when total is null", () => {
    assert.equal(rate({ exploratory: 87, client: 20, total: null }), 0.87);
  });

  it("[R33] is null when total and exploratory are both null, even with client set", () => {
    assert.equal(rate({ exploratory: null, client: 50, total: null }), null);
    assert.equal(rate({ exploratory: null, client: null, total: null }), null);
  });

  it("[R33] a total of 0 is a rate of 0, not a missing figure", () => {
    assert.equal(rate({ exploratory: 90, client: 90, total: 0 }), 0);
  });
});

describe("[R33] statsView tunnels and counts", () => {
  it("[R33] maps tunnels.in, out, participating, client and exploratory", () => {
    const view = statsView(
      stats({ tunnels: { in: 1, out: 2, participating: 3, client: 4, exploratory: 5 } }),
    );
    assert.equal(view.inboundTunnels, 1);
    assert.equal(view.outboundTunnels, 2);
    assert.equal(view.participatingTunnels, 3);
    assert.equal(view.clientTunnels, 4);
    assert.equal(view.exploratoryTunnels, 5);
  });

  it("[R33] keeps a null tunnel count null and a zero count zero", () => {
    const view = statsView(
      stats({ tunnels: { in: null, out: 0, participating: 0, client: null, exploratory: 0 } }),
    );
    assert.equal(view.inboundTunnels, null);
    assert.equal(view.outboundTunnels, 0);
    assert.equal(view.participatingTunnels, 0);
    assert.equal(view.clientTunnels, null);
    assert.equal(view.exploratoryTunnels, 0);
  });
});

describe("[R33] statsView counts and status", () => {
  it("[R33] maps networkStatus, activePeers, knownRouters, floodfills and version", () => {
    const view = statsView(
      stats({
        networkStatus: "FIREWALLED",
        activePeers: 7,
        knownRouters: 8,
        floodfills: 9,
        version: "2.10.0",
      }),
    );
    assert.equal(view.networkStatus, "FIREWALLED");
    assert.equal(view.activePeers, 7);
    assert.equal(view.knownRouters, 8);
    assert.equal(view.floodfills, 9);
    assert.equal(view.routerVersion, "2.10.0");
  });

  it("[R33] keeps a zero bandwidth, which is a figure and not a missing one", () => {
    const view = statsView(
      stats({ bandwidthBytesPerSecond: { in1s: 0, out1s: 0, in5m: null, out5m: null } }),
    );
    assert.equal(view.bandwidthInBps, 0);
    assert.equal(view.bandwidthOutBps, 0);
  });
});

describe("[R33] statsView history", () => {
  it("[R33] an empty history gives a null history", () => {
    assert.equal(statsView(stats({ history: [] })).history, null);
  });

  it("[R33] a history gives its in and out values, oldest first, in steps of 5 seconds", () => {
    const view = statsView(
      stats({
        history: [
          { t: 100, in: 1, out: 9 },
          { t: 5100, in: 2, out: 8 },
        ],
      }),
    );
    assert.deepEqual(view.history, { stepSeconds: 5, inBps: [1, 2], outBps: [9, 8] });
  });

  it("[R33] a single sample is a history", () => {
    const view = statsView(stats({ history: [{ t: 1, in: 5, out: 6 }] }));
    assert.deepEqual(view.history, { stepSeconds: 5, inBps: [5], outBps: [6] });
  });
});
