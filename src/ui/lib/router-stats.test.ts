// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// Requirement tests for R41 of the router console wiki page ("The UI reads the contract
// shape"): `statsView` turns the `RouterStats` of the IPC contract v1.7 into the view that
// the router panel and the Network page show.

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { RouterStats } from "../contract.ts";
import { HISTORY_SLOTS, HISTORY_STEP_SECONDS, type StatsView, statsView } from "./router-stats.ts";

/** The time now, in Unix ms, that every test passes to `statsView`. */
const NOW = 1_700_000_000_000;
const STEP_MS = 5000;

/** 120 slots, all null, with the given `[slot, value]` pairs set. */
const slotsOf = (...entries: [number, number][]): (number | null)[] => {
  const slots: (number | null)[] = Array.from({ length: 120 }, () => null);
  for (const [slot, value] of entries) slots[slot] = value;
  return slots;
};

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
    { t: NOW - 10_000, in: 10, out: 20 },
    { t: NOW - 5000, in: 30, out: 40 },
    { t: NOW, in: 50, out: 60 },
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
  history: {
    stepSeconds: 5,
    inBps: slotsOf([117, 10], [118, 30], [119, 50]),
    outBps: slotsOf([117, 20], [118, 40], [119, 60]),
  },
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
    assert.deepEqual(statsView(FULL, NOW), FULL_VIEW);
  });

  it("[R41] shows the shortest bandwidth window, in1s and out1s, not the 5 minute one", () => {
    const view = statsView(FULL, NOW);
    assert.equal(view.bandwidthInBps, 12_636);
    assert.equal(view.bandwidthOutBps, 5806);
  });

  it("[R41] gives the routerKind and the javaVersion as null", () => {
    const view = statsView(FULL, NOW);
    assert.equal(view.routerKind, null);
    assert.equal(view.javaVersion, null);
  });

  it("[R41] the history step is 5 seconds", () => {
    assert.equal(HISTORY_STEP_SECONDS, 5);
    assert.equal(statsView(FULL, NOW).history?.stepSeconds, 5);
  });
});

describe("[R41] statsView with a missing input", () => {
  it("[R41] a null input gives a view with every field null", () => {
    assert.deepEqual(statsView(null, NOW), NULL_VIEW);
  });

  it("[R41] an undefined input gives a view with every field null", () => {
    assert.deepEqual(statsView(undefined, NOW), NULL_VIEW);
  });

  it("[R41] an answer with every field null gives a view with every field null", () => {
    assert.deepEqual(statsView(emptyStats(), NOW), NULL_VIEW);
  });
});

describe("[R41] statsView uptime", () => {
  it("[R41] uptimeSeconds is floor(uptimeMs / 1000)", () => {
    assert.equal(statsView(stats({ uptimeMs: 28_800_999 }), NOW).uptimeSeconds, 28_800);
    assert.equal(statsView(stats({ uptimeMs: 999 }), NOW).uptimeSeconds, 0);
    assert.equal(statsView(stats({ uptimeMs: 0 }), NOW).uptimeSeconds, 0);
    assert.equal(statsView(stats({ uptimeMs: null }), NOW).uptimeSeconds, null);
  });

  it("[R41] uptimeResolutionSeconds is max(1, floor(uptimeResolutionMs / 1000))", () => {
    const resolution = (ms: number | null): number | null =>
      statsView(stats({ uptimeResolutionMs: ms }), NOW).uptimeResolutionSeconds;
    assert.equal(resolution(3_600_000), 3600);
    assert.equal(resolution(86_400_000), 86_400);
    assert.equal(resolution(60_000), 60);
    assert.equal(resolution(2500), 2);
    assert.equal(resolution(1000), 1);
  });

  it("[R41] a resolution under one second is at least 1 second", () => {
    const resolution = (ms: number): number | null =>
      statsView(stats({ uptimeResolutionMs: ms }), NOW).uptimeResolutionSeconds;
    assert.equal(resolution(1), 1);
    assert.equal(resolution(999), 1);
    assert.equal(resolution(0), 1);
  });

  it("[R41] a null uptimeResolutionMs gives a null uptimeResolutionSeconds", () => {
    assert.equal(statsView(stats({ uptimeResolutionMs: null }), NOW).uptimeResolutionSeconds, null);
  });
});

describe("[R41] statsView build success", () => {
  const rate = (build: RouterStats["tunnelBuildSuccessPercent"]): number | null =>
    statsView(stats({ tunnelBuildSuccessPercent: build }), NOW).buildSuccessRate;

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
      NOW,
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
      NOW,
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
      NOW,
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
      NOW,
    );
    assert.equal(view.bandwidthInBps, 0);
    assert.equal(view.bandwidthOutBps, 0);
  });
});

type Sample = RouterStats["history"][number];

/** A sample at `t` ms whose in and out are `value` and `value + 1`, so a test reads which one stays. */
const sampleAt = (t: number, value = 1): Sample => ({ t, in: value, out: value + 1 });

/** The in series of the view of `history`, or null when the view has no history. */
const inSlots = (history: Sample[], nowMs = NOW): (number | null)[] | null =>
  statsView(stats({ history }), nowMs).history?.inBps ?? null;

const outSlots = (history: Sample[], nowMs = NOW): (number | null)[] | null =>
  statsView(stats({ history }), nowMs).history?.outBps ?? null;

/** Samples at `NOW - back` ms for each `back`, with the in value of its position, newest first. */
const samplesBack = (...back: number[]): Sample[] => back.map((ms, n) => sampleAt(NOW - ms, n + 1));

describe("[R41] statsView history slots", () => {
  it("[R41] the history has 120 slots of 5 seconds", () => {
    assert.equal(HISTORY_SLOTS, 120);
    const history = statsView(stats({ history: [sampleAt(NOW)] }), NOW).history;
    assert.equal(history?.stepSeconds, 5);
    assert.equal(history?.inBps.length, 120);
    assert.equal(history?.outBps.length, 120);
  });

  it("[R41] slot 119 is now and slot 0 is 595 seconds ago, oldest first", () => {
    const history = [sampleAt(NOW - 119 * STEP_MS, 7), sampleAt(NOW, 9)];
    assert.deepEqual(inSlots(history), slotsOf([0, 7], [119, 9]));
    assert.deepEqual(outSlots(history), slotsOf([0, 8], [119, 10]));
  });

  it("[R41] a sample every 5 seconds fills slot 0 to slot 119 with no gap", () => {
    const history = Array.from({ length: 120 }, (_, n) => sampleAt(NOW - (119 - n) * STEP_MS, n));
    assert.deepEqual(
      inSlots(history),
      Array.from({ length: 120 }, (_, n) => n),
    );
  });

  it("[R41] the time now comes from the caller: the same sample moves one slot a step later", () => {
    const history = [sampleAt(NOW, 3)];
    assert.deepEqual(inSlots(history, NOW), slotsOf([119, 3]));
    assert.deepEqual(inSlots(history, NOW + STEP_MS), slotsOf([118, 3]));
    assert.deepEqual(inSlots(history, NOW + 10 * STEP_MS), slotsOf([109, 3]));
  });
});

describe("[R41] statsView history slot of the newest sample", () => {
  it("[R41] the newest sample goes to slot 119 - Math.round((nowMs - t) / 5000)", () => {
    assert.deepEqual(inSlots([sampleAt(NOW - 2400)]), slotsOf([119, 1]));
    assert.deepEqual(inSlots([sampleAt(NOW - 2500)]), slotsOf([118, 1]));
    assert.deepEqual(inSlots([sampleAt(NOW - 7400)]), slotsOf([118, 1]));
    assert.deepEqual(inSlots([sampleAt(NOW - 7500)]), slotsOf([117, 1]));
    assert.deepEqual(inSlots([sampleAt(NOW - 40 * STEP_MS, 2)]), slotsOf([79, 2]));
  });

  it("[R41] a sample up to 2 500 ms after now goes to slot 119", () => {
    assert.deepEqual(inSlots([sampleAt(NOW + 2000)]), slotsOf([119, 1]));
    assert.deepEqual(inSlots([sampleAt(NOW + 2500)]), slotsOf([119, 1]));
  });

  it("[R41] a sample with t above nowMs + 2 500 is dropped", () => {
    assert.equal(inSlots([sampleAt(NOW + 2501)]), null);
    assert.equal(inSlots([sampleAt(NOW + STEP_MS)]), null);
  });

  it("[R41] the newest sample that is not dropped gets the slot, not the dropped one", () => {
    const history = [sampleAt(NOW + 3000, 8), sampleAt(NOW - 1000, 4)];
    assert.deepEqual(inSlots(history), slotsOf([119, 4]));
  });
});

describe("[R41] statsView history slots from the newest sample back", () => {
  it("[R41] samples 5 300 ms apart fill every slot, with no null", () => {
    const history = Array.from({ length: 120 }, (_, n) => sampleAt(NOW - (119 - n) * 5300, n));
    assert.deepEqual(
      inSlots(history),
      Array.from({ length: 120 }, (_, n) => n),
    );
  });

  it("[R41] a late sampler never leaves an empty slot: gaps up to 10 000 ms are one slot", () => {
    const history = samplesBack(0, 5300, 10_300, 16_400, 21_300, 31_300);
    assert.deepEqual(
      inSlots(history),
      slotsOf([119, 1], [118, 2], [117, 3], [116, 4], [115, 5], [114, 6]),
    );
  });
});

describe("[R41] statsView history slots across a gap", () => {
  it("[R41] a gap of exactly 10 000 ms is one slot, a gap of 10 001 ms is two", () => {
    assert.deepEqual(inSlots(samplesBack(0, 10_000)), slotsOf([119, 1], [118, 2]));
    assert.deepEqual(inSlots(samplesBack(0, 10_001)), slotsOf([119, 1], [117, 2]));
  });

  it("[R41] a gap above 10 000 ms is Math.round(gap / 5000) slots", () => {
    assert.deepEqual(inSlots(samplesBack(0, 12_400)), slotsOf([119, 1], [117, 2]));
    assert.deepEqual(inSlots(samplesBack(0, 12_500)), slotsOf([119, 1], [116, 2]));
  });

  it("[R41] a 15 second gap leaves 2 null slots", () => {
    const slots = inSlots(samplesBack(0, 15_000));
    assert.deepEqual(slots, slotsOf([119, 1], [116, 2]));
    assert.equal(slots?.[118], null);
    assert.equal(slots?.[117], null);
  });

  it("[R41] the gap counts from the next newer sample, so a late sample moves the older ones", () => {
    const history = samplesBack(2400, 7400, 30_000);
    assert.deepEqual(inSlots(history), slotsOf([119, 1], [118, 2], [113, 3]));
  });

  it("[R41] two samples close in time are two slots: no sample replaces another", () => {
    const history = [sampleAt(NOW - 1000, 10), sampleAt(NOW, 20)];
    assert.deepEqual(inSlots(history), slotsOf([118, 10], [119, 20]));
    assert.deepEqual(outSlots(history), slotsOf([118, 11], [119, 21]));
  });

  it("[R41] a history in any order gives the same slots", () => {
    const sorted = samplesBack(0, 5300, 21_000, 26_000);
    const shuffled = [sorted[2], sorted[0], sorted[3], sorted[1]].filter(
      (s): s is Sample => s !== undefined,
    );
    assert.equal(shuffled.length, 4);
    assert.deepEqual(inSlots(shuffled), inSlots(sorted));
    assert.deepEqual(inSlots(shuffled), slotsOf([119, 1], [118, 2], [115, 3], [114, 4]));
  });
});

describe("[R41] statsView history drops the samples below slot 0", () => {
  it("[R41] a sample whose slot is below 0 is dropped, and every older one with it", () => {
    const history = samplesBack(0, 5000, 119 * STEP_MS, 120 * STEP_MS, 121 * STEP_MS);
    const slots = inSlots(history);
    assert.deepEqual(slots, slotsOf([119, 1], [118, 2], [0, 3]));
  });

  it("[R41] an older sample after a big gap is dropped too once the slots are below 0", () => {
    const history = samplesBack(0, 125 * STEP_MS, 126 * STEP_MS + 1000);
    assert.deepEqual(inSlots(history), slotsOf([119, 1]));
  });

  it("[R41] a newest sample older than 10 minutes gives a null history", () => {
    assert.equal(inSlots([sampleAt(NOW - 120 * STEP_MS)]), null);
    assert.equal(inSlots(samplesBack(3_600_000, 3_605_000)), null);
  });
});

describe("[R41] statsView history keeps the samples as they are", () => {
  it("[R41] a slot with no sample is null, in both arrays", () => {
    const view = statsView(stats({ history: samplesBack(0, 15_000) }), NOW).history;
    assert.equal(view?.inBps[118], null);
    assert.equal(view?.outBps[118], null);
    assert.equal(view?.inBps.filter((v) => v !== null).length, 2);
    assert.equal(view?.outBps.filter((v) => v !== null).length, 2);
  });

  it("[R41] a gap in the samples stays a gap: nothing is added, repeated or interpolated", () => {
    const history = [sampleAt(NOW - 100 * STEP_MS, 4), sampleAt(NOW - 10 * STEP_MS, 8)];
    const slots = inSlots(history);
    assert.deepEqual(slots, slotsOf([19, 4], [109, 8]));
    assert.equal(slots?.filter((v) => v !== null).length, 2);
  });

  it("[R41] keeps the in and out of each sample unchanged, zero included", () => {
    const history: Sample[] = [
      { t: NOW - STEP_MS, in: 33, out: 44 },
      { t: NOW, in: 0, out: 0 },
    ];
    const view = statsView(stats({ history }), NOW).history;
    assert.deepEqual(view?.inBps, slotsOf([118, 33], [119, 0]));
    assert.deepEqual(view?.outBps, slotsOf([118, 44], [119, 0]));
  });
});

describe("[R41] statsView history is null when no sample gets a slot", () => {
  it("[R41] an empty history gives a null history", () => {
    assert.equal(statsView(stats({ history: [] }), NOW).history, null);
  });

  it("[R41] a history whose samples are all out of range gives a null history", () => {
    const history = [sampleAt(NOW - 200 * STEP_MS), sampleAt(NOW + 10 * STEP_MS)];
    assert.equal(statsView(stats({ history }), NOW).history, null);
  });
});
