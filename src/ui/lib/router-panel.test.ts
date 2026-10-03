// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  formatUptime,
  type PanelStatsLike,
  type PanelStatusLike,
  panelControls,
  panelState,
  panelText,
  recentWindow,
  sparkSeries,
} from "./router-panel.ts";

const status = (patch: Partial<PanelStatusLike> = {}): PanelStatusLike => ({
  state: "ok",
  proxy: "127.0.0.1:4444",
  version: "2.10.0",
  detail: null,
  managed: true,
  paused: false,
  ...patch,
});

describe("panelControls", () => {
  it("offers pause while browsing and resume while paused", () => {
    assert.equal(panelControls(status()).pauseCommand, "connection_pause");
    assert.equal(panelControls(status()).pauseLabel, "Pause I2P browsing");
    assert.equal(panelControls(status({ paused: true })).pauseCommand, "connection_resume");
  });
});

const stats = (patch: Partial<PanelStatsLike> = {}): PanelStatsLike => ({
  routerVersion: "2.10.0",
  uptimeSeconds: 7380,
  activePeers: 42,
  inboundTunnels: 7,
  outboundTunnels: 8,
  participatingTunnels: 345,
  buildSuccessRate: 0.87,
  bandwidthInBps: 2048,
  bandwidthOutBps: 1_000_000,
  ...patch,
});
const EMPTY: PanelStatsLike = {
  routerVersion: null,
  uptimeSeconds: null,
  activePeers: null,
  inboundTunnels: null,
  outboundTunnels: null,
  participatingTunnels: null,
  buildSuccessRate: null,
  bandwidthInBps: null,
  bandwidthOutBps: null,
};

describe("Router panel figures", () => {
  it("Router panel: shows version, uptime, peers, tunnels, build rate and bandwidth", () => {
    const text = panelText(stats());
    assert.match(text.version, /2\.10\.0/);
    assert.match(text.uptime, /\d/);
    assert.notEqual(text.uptime, "—");
    assert.match(text.peers, /42/);
    assert.match(text.build, /87/);
    assert.notEqual(text.bandwidth, "—");
  });
  it("Router panel: shows the inbound, outbound and participating tunnels", () => {
    const { tunnels } = panelText(stats());
    for (const count of ["7", "8", "345"]) {
      assert.ok(tunnels.includes(count), `${tunnels} must show ${count}`);
    }
  });
  it('Router panel: shows a missing figure as "—"', () => {
    const text = panelText(EMPTY);
    for (const [name, value] of Object.entries(text)) {
      assert.ok(value.includes("—"), `${name} is "${value}", not "—"`);
    }
  });
});

describe("Router panel state", () => {
  const status = (state: PanelStatusLike["state"]): PanelStatusLike => ({
    state,
    proxy: "127.0.0.1:4444",
    version: "2.10.0",
    detail: null,
    managed: true,
    paused: false,
  });
  it("Router panel: shows a different colored dot for ready, building and stopped", () => {
    const tones = [status("ok"), status("building"), status("down")].map((s) => panelState(s).tone);
    assert.deepEqual(tones, ["ready", "building", "stopped"]);
  });
  it("Router panel: explains the state in one line", () => {
    for (const state of ["ok", "building", "down"] as const) {
      const { text } = panelState(status(state));
      assert.ok(text.trim().length > 0, `${state} has an explanation`);
      assert.equal(text.includes("\n"), false);
    }
  });
});

describe("Router panel buttons", () => {
  const unmanaged = { ...status(), managed: false };
  it("Router panel: turns Restart and Stop off for a router eepview does not manage", () => {
    const controls = panelControls(unmanaged);
    assert.equal(controls.routerEnabled, false);
    assert.equal(controls.routerTitle, "Only for a router that eepview manages");
  });
  it("Router panel: keeps Restart and Stop on for a router eepview manages", () => {
    assert.equal(panelControls(status()).routerEnabled, true);
  });
});

describe("Router panel sparkline", () => {
  // One sample per step: n samples span (n - 1) * step seconds, so 10 minutes is 120 or 121.
  const spanSeconds = (samples: readonly number[], step: number): number =>
    (samples.length - 1) * step;
  it("Router panel: covers the last 10 minutes", () => {
    const values = Array.from({ length: 200 }, (_, n) => n);
    for (const step of [5, 10]) {
      const kept = recentWindow(values, step);
      assert.ok(spanSeconds(kept, step) <= 600, `step ${step} keeps too much`);
      assert.ok(spanSeconds(kept, step) >= 600 - step, `step ${step} drops recent samples`);
      assert.equal(kept[kept.length - 1], 199);
    }
  });
  it("Router panel: keeps a history shorter than 10 minutes whole", () => {
    assert.deepEqual(recentWindow([1, 2, 3], 5), [1, 2, 3]);
  });
  it("Router panel: trims both directions of the history to 10 minutes", () => {
    const long = Array.from({ length: 300 }, (_, n) => n);
    const series = sparkSeries({ stepSeconds: 5, inBps: long, outBps: long });
    assert.ok((series?.inBps.length ?? 0) <= 121);
    assert.ok((series?.outBps.length ?? 0) <= 121);
    assert.equal(sparkSeries(null), null);
  });
});

describe("[R34] the router panel shows the uptime to its resolution", () => {
  it('[R34] 28 800 s with a resolution of 3 600 s reads "8 h"', () => {
    const text = panelText(stats({ uptimeSeconds: 28_800, uptimeResolutionSeconds: 3600 }));
    assert.equal(text.uptime, "8 h");
  });

  it('[R34] 172 800 s with a resolution of 86 400 s reads "2 d"', () => {
    const text = panelText(stats({ uptimeSeconds: 172_800, uptimeResolutionSeconds: 86_400 }));
    assert.equal(text.uptime, "2 d");
  });

  it("[R34] a null or missing resolution keeps today's output", () => {
    const today = formatUptime(28_800);
    assert.equal(today, "8 h 0 min");
    assert.equal(panelText(stats({ uptimeSeconds: 28_800 })).uptime, today);
    const nulled = panelText(stats({ uptimeSeconds: 28_800, uptimeResolutionSeconds: null }));
    assert.equal(nulled.uptime, today);
  });

  it("[R34] formatUptime in the panel module follows the same rule", () => {
    assert.equal(formatUptime(28_800, 3600), "8 h");
    assert.equal(formatUptime(172_800, 86_400), "2 d");
    assert.equal(formatUptime(7380, 30), "2 h 3 min");
  });
});

const tunnelsLine = (patch: Partial<PanelStatsLike>): string => panelText(stats(patch)).tunnels;
const NO_SPLIT = { inboundTunnels: null, outboundTunnels: null };
const NO_PARTICIPATING = { clientTunnels: 2, exploratoryTunnels: 11, participatingTunnels: null };

describe("[R35] the tunnels line without the in and out split", () => {
  it("[R35] shows client, exploratory and participating", () => {
    assert.equal(
      tunnelsLine({
        ...NO_SPLIT,
        clientTunnels: 2,
        exploratoryTunnels: 11,
        participatingTunnels: 398,
      }),
      "2 client · 11 exploratory · 398 participating",
    );
  });

  it('[R35] shows "—" for a client count that is null', () => {
    const line = tunnelsLine({ ...NO_SPLIT, exploratoryTunnels: 11, participatingTunnels: 398 });
    assert.equal(line, "— client · 11 exploratory · 398 participating");
  });

  it('[R35] shows "—" for an exploratory count that is null', () => {
    const line = tunnelsLine({ ...NO_SPLIT, clientTunnels: 2, participatingTunnels: 398 });
    assert.equal(line, "2 client · — exploratory · 398 participating");
  });

  it('[R35] shows "—" for a participating count that is null', () => {
    const line = tunnelsLine({ ...NO_SPLIT, ...NO_PARTICIPATING });
    assert.equal(line, "2 client · 11 exploratory · — participating");
  });

  it("[R35] uses the client form when only one of client and exploratory is set", () => {
    assert.match(tunnelsLine({ ...NO_SPLIT, clientTunnels: 2, participatingTunnels: 5 }), /client/);
    const exploratory = tunnelsLine({
      ...NO_SPLIT,
      exploratoryTunnels: 3,
      participatingTunnels: 5,
    });
    assert.match(exploratory, /exploratory/);
  });
});

describe("[R35] the tunnels line keeps the in and out form", () => {
  it("[R35] when in and out are set", () => {
    assert.equal(
      tunnelsLine({ inboundTunnels: 7, outboundTunnels: 8, participatingTunnels: 345 }),
      "7 in · 8 out · 345 participating",
    );
  });

  it("[R35] when in and out are set, even with client and exploratory", () => {
    const line = tunnelsLine({
      inboundTunnels: 7,
      outboundTunnels: 8,
      participatingTunnels: 345,
      clientTunnels: 2,
      exploratoryTunnels: 3,
    });
    assert.equal(line, "7 in · 8 out · 345 participating");
  });

  it("[R35] when only one of in and out is null", () => {
    const base = { participatingTunnels: 345, clientTunnels: 2, exploratoryTunnels: 3 };
    assert.equal(
      tunnelsLine({ ...base, inboundTunnels: 7, outboundTunnels: null }),
      "7 in · — out · 345 participating",
    );
    assert.equal(
      tunnelsLine({ ...base, inboundTunnels: null, outboundTunnels: 8 }),
      "— in · 8 out · 345 participating",
    );
  });

  it("[R35] when client and exploratory are null or missing", () => {
    const line = "— in · — out · 345 participating";
    assert.equal(tunnelsLine({ ...NO_SPLIT, participatingTunnels: 345 }), line);
    const nulled = { ...NO_SPLIT, clientTunnels: null, exploratoryTunnels: null };
    assert.equal(tunnelsLine({ ...nulled, participatingTunnels: 345 }), line);
  });
});
