// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// Requirement tests for R41 of the router console wiki page ("The UI reads the contract
// shape"): `statsView` turns the `RouterStats` of the IPC contract v1.7 into the view that
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

/** A contract v1.7 answer with every field null and no history. */
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

describe("[R41] statsView maps the contract shape to the view", () => {
  it("[R41] gives every view field from its RouterStats field", () => {
    assert.deepEqual(statsView(FULL), FULL_VIEW);
  });

  it("[R41] shows the shortest bandwidth window, in1s and out1s, not the 5 minute one", () => {
    const view = statsView(FULL);
    assert.equal(view.bandwidthInBps, 12_636);
    assert.equal(view.bandwidthOutBps, 5806);
  });

  it("[R41] gives the routerKind and the javaVersion as null", () => {
    const view = statsView(FULL);
    assert.equal(view.routerKind, null);
    assert.equal(view.javaVersion, null);
  });

  it("[R41] the history step is 5 seconds", () => {
    assert.equal(HISTORY_STEP_SECONDS, 5);
    assert.equal(statsView(FULL).history?.stepSeconds, 5);
  });
});

describe("[R41] statsView with a missing input", () => {
  it("[R41] a null input gives a view with every field null", () => {
    assert.deepEqual(statsView(null), NULL_VIEW);
  });

  it("[R41] an undefined input gives a view with every field null", () => {
    assert.deepEqual(statsView(undefined), NULL_VIEW);
  });

  it("[R41] an answer with every field null gives a view with every field null", () => {
    assert.deepEqual(statsView(emptyStats()), NULL_VIEW);
  });
});

describe("[R41] statsView uptime", () => {
  it("[R41] uptimeSeconds is floor(uptimeMs / 1000)", () => {
    assert.equal(statsView(stats({ uptimeMs: 28_800_999 })).uptimeSeconds, 28_800);
    assert.equal(statsView(stats({ uptimeMs: 999 })).uptimeSeconds, 0);
    assert.equal(statsView(stats({ uptimeMs: 0 })).uptimeSeconds, 0);
    assert.equal(statsView(stats({ uptimeMs: null })).uptimeSeconds, null);
  });

  it("[R41] uptimeResolutionSeconds is max(1, floor(uptimeResolutionMs / 1000))", () => {
    const resolution = (ms: number | null): number | null =>
      statsView(stats({ uptimeResolutionMs: ms })).uptimeResolutionSeconds;
    assert.equal(resolution(3_600_000), 3600);
    assert.equal(resolution(86_400_000), 86_400);
    assert.equal(resolution(60_000), 60);
    assert.equal(resolution(2500), 2);
    assert.equal(resolution(1000), 1);
  });

  it("[R41] a resolution under one second is at least 1 second", () => {
    const resolution = (ms: number): number | null =>
      statsView(stats({ uptimeResolutionMs: ms })).uptimeResolutionSeconds;
    assert.equal(resolution(1), 1);
    assert.equal(resolution(999), 1);
    assert.equal(resolution(0), 1);
  });

  it("[R41] a null uptimeResolutionMs gives a null uptimeResolutionSeconds", () => {
    assert.equal(statsView(stats({ uptimeResolutionMs: null })).uptimeResolutionSeconds, null);
  });
});

describe("[R41] statsView build success", () => {
  const rate = (build: RouterStats["tunnelBuildSuccessPercent"]): number | null =>
    statsView(stats({ tunnelBuildSuccessPercent: build })).buildSuccessRate;

  it("[R41] is total / 100 when total is set", () => {
    assert.equal(rate({ exploratory: 10, client: 20, total: 42 }), 0.42);
    assert.equal(rate({ exploratory: null, client: null, total: 100 }), 1);
  });

  it("[R41] is exploratory / 100 when total is null", () => {
    assert.equal(rate({ exploratory: 87, client: 20, total: null }), 0.87);
  });

  it("[R41] is null when total and exploratory are both null, even with client set", () => {
    assert.equal(rate({ exploratory: null, client: 50, total: null }), null);
    assert.equal(rate({ exploratory: null, client: null, total: null }), null);
  });

  it("[R41] a total of 0 is a rate of 0, not a missing figure", () => {
    assert.equal(rate({ exploratory: 90, client: 90, total: 0 }), 0);
  });
});

describe("[R41] statsView tunnels and counts", () => {
  it("[R41] maps tunnels.in, out, participating, client and exploratory", () => {
    const view = statsView(
      stats({ tunnels: { in: 1, out: 2, participating: 3, client: 4, exploratory: 5 } }),
    );
    assert.equal(view.inboundTunnels, 1);
    assert.equal(view.outboundTunnels, 2);
    assert.equal(view.participatingTunnels, 3);
    assert.equal(view.clientTunnels, 4);
    assert.equal(view.exploratoryTunnels, 5);
  });

  it("[R41] keeps a null tunnel count null and a zero count zero", () => {
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

describe("[R41] statsView counts and status", () => {
  it("[R41] maps networkStatus, activePeers, knownRouters, floodfills and version", () => {
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

  it("[R41] keeps a zero bandwidth, which is a figure and not a missing one", () => {
    const view = statsView(
      stats({ bandwidthBytesPerSecond: { in1s: 0, out1s: 0, in5m: null, out5m: null } }),
    );
    assert.equal(view.bandwidthInBps, 0);
    assert.equal(view.bandwidthOutBps, 0);
  });
});

describe("[R41] statsView history", () => {
  it("[R41] an empty history gives a null history", () => {
    assert.equal(statsView(stats({ history: [] })).history, null);
  });

  it("[R41] a history gives its in and out values, oldest first, in steps of 5 seconds", () => {
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

  it("[R41] a single sample is a history", () => {
    const view = statsView(stats({ history: [{ t: 1, in: 5, out: 6 }] }));
    assert.deepEqual(view.history, { stepSeconds: 5, inBps: [5], outBps: [6] });
  });
});

type Sample = RouterStats["history"][number];

/** A sample at `t` ms whose in and out are `t / 1000` and `t / 1000 + 1`, so a test reads which sample was kept. */
const sampleAt = (t: number): Sample => ({ t, in: t / 1000, out: t / 1000 + 1 });

const runOf = (times: number[]): StatsView["history"] =>
  statsView(stats({ history: times.map(sampleAt) })).history;

const historyOf = (times: number[]): StatsView["history"] => ({
  stepSeconds: 5,
  inBps: times.map((t) => t / 1000),
  outBps: times.map((t) => t / 1000 + 1),
});

describe("[R41] statsView history run", () => {
  it("[R41] a gap above 10 000 ms ends the run: only the samples after it are kept", () => {
    assert.deepEqual(runOf([0, 5000, 10_000, 20_001, 25_001]), historyOf([20_001, 25_001]));
  });

  it("[R41] a gap of 10 001 ms ends the run, a gap of 10 000 ms does not", () => {
    assert.deepEqual(runOf([0, 10_001]), historyOf([10_001]));
    assert.deepEqual(runOf([0, 10_000]), historyOf([0, 10_000]));
  });

  it("[R41] a gap of exactly 10 000 ms stays in the run", () => {
    assert.deepEqual(runOf([0, 10_000, 20_000, 30_000]), historyOf([0, 10_000, 20_000, 30_000]));
  });

  it("[R41] a gap just under 10 000 ms stays in the run", () => {
    assert.deepEqual(runOf([0, 9999]), historyOf([0, 9999]));
  });

  it("[R41] keeps only the last run when there are several gaps", () => {
    const times = [0, 5000, 20_000, 25_000, 40_000, 45_000, 50_000];
    assert.deepEqual(runOf(times), historyOf([40_000, 45_000, 50_000]));
  });

  it("[R41] a gap before the newest sample leaves that one sample as the run", () => {
    assert.deepEqual(runOf([0, 5000, 10_000, 30_000]), historyOf([30_000]));
  });
});

describe("[R41] statsView history run edge cases", () => {
  it("[R41] a single sample is a run", () => {
    assert.deepEqual(runOf([7000]), historyOf([7000]));
  });

  it("[R41] an empty history gives a null history", () => {
    assert.equal(runOf([]), null);
  });

  it("[R41] a history with no gap is kept whole, oldest first", () => {
    const times = [1000, 6000, 11_000, 16_000, 21_000];
    assert.deepEqual(runOf(times), historyOf(times));
  });
});

describe("[R41] statsView history run keeps samples as they are", () => {
  it("[R41] never adds, repeats or interpolates a sample", () => {
    const view = runOf([0, 5000, 25_000, 30_000]);
    assert.deepEqual(view, historyOf([25_000, 30_000]));
    assert.equal(view?.inBps.length, 2);
    assert.equal(view?.outBps.length, 2);
  });

  it("[R41] keeps the in and out of each kept sample unchanged", () => {
    const history: Sample[] = [
      { t: 0, in: 1, out: 2 },
      { t: 30_000, in: 33, out: 44 },
      { t: 35_000, in: 0, out: 0 },
    ];
    const view = statsView(stats({ history })).history;
    assert.deepEqual(view, { stepSeconds: 5, inBps: [33, 0], outBps: [44, 0] });
  });

  it("[R41] the run rule uses the time of the samples, not their number", () => {
    const times = [0, 1000, 2000, 3000, 4000, 5000, 6000, 7000, 8000, 9000, 10_000, 11_000];
    assert.deepEqual(runOf(times), historyOf(times));
  });
});
