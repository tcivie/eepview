// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  bandwidthText,
  type PanelStatsLike,
  type PanelStatusLike,
  panelControls,
  panelState,
  panelText,
  recentWindow,
  SPARK_WINDOW_SECONDS,
  sparkSeries,
  tunnelsText,
  UNMANAGED_TITLE,
} from "./router-panel.ts";
import { MISSING } from "./stats-view.ts";

const status = (patch: Partial<PanelStatusLike> = {}): PanelStatusLike => ({
  state: "ok",
  proxy: "127.0.0.1:4444",
  version: "2.10.0",
  detail: null,
  managed: true,
  paused: false,
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

describe("panelText", () => {
  it("formats every figure", () => {
    const text = panelText({
      routerVersion: "2.10.0",
      uptimeSeconds: 11_520,
      activePeers: 187,
      inboundTunnels: 3,
      outboundTunnels: 3,
      participatingTunnels: 1214,
      buildSuccessRate: 0.68,
      bandwidthInBps: 46_080,
      bandwidthOutBps: 2048,
    });
    assert.deepEqual(text, {
      version: "I2P 2.10.0",
      uptime: "3 h 12 min",
      peers: "187",
      tunnels: "3 in · 3 out · 1,214 participating",
      build: "68%",
      bandwidth: "45.0 KB/s in · 2.0 KB/s out",
    });
  });
  it("shows a dash for every missing figure", () => {
    const text = panelText(EMPTY);
    assert.equal(text.version, MISSING);
    assert.equal(text.peers, MISSING);
    assert.equal(text.build, MISSING);
    assert.equal(
      tunnelsText(null, null, null),
      `${MISSING} in · ${MISSING} out · ${MISSING} participating`,
    );
    assert.equal(bandwidthText(null, null), `${MISSING} in · ${MISSING} out`);
  });
});

describe("panelState", () => {
  it("explains the router state in one line, without the version", () => {
    const state = panelState(status());
    assert.equal(state.tone, "ready");
    assert.equal(state.label, "Router ready");
    assert.equal(state.text, "Eepsites open through your I2P router.");
  });
  it("prefers the shell's detail", () => {
    assert.equal(
      panelState(status({ state: "down", detail: "Exit code 1." })).text,
      "Exit code 1.",
    );
  });
  it("says when browsing is paused", () => {
    const state = panelState(status({ paused: true }));
    assert.deepEqual([state.tone, state.label], ["stopped", "Paused"]);
  });
});

describe("panelControls", () => {
  it("offers pause while browsing and resume while paused", () => {
    assert.equal(panelControls(status()).pauseCommand, "connection_pause");
    assert.equal(panelControls(status()).pauseLabel, "Pause I2P browsing");
    assert.equal(panelControls(status({ paused: true })).pauseCommand, "connection_resume");
  });
  it("disables restart and stop for a router eepview does not manage", () => {
    assert.deepEqual(panelControls(status({ managed: false })), {
      pauseLabel: "Pause I2P browsing",
      pauseCommand: "connection_pause",
      routerEnabled: false,
      routerTitle: UNMANAGED_TITLE,
    });
    assert.equal(panelControls(status()).routerTitle, null);
  });
});

describe("recentWindow", () => {
  const series = Array.from({ length: 200 }, (_, i) => i);
  it("keeps the last ten minutes of samples", () => {
    const window = recentWindow(series, 10);
    assert.equal(window.length, SPARK_WINDOW_SECONDS / 10 + 1);
    assert.equal(window[window.length - 1], 199);
  });
  it("keeps at least two points and copes with a bad step", () => {
    assert.equal(recentWindow(series, 3600).length, 2);
    assert.equal(recentWindow(series, 0).length, 200);
  });
});

describe("sparkSeries", () => {
  it("has nothing to draw without a history or with fewer than two samples", () => {
    assert.equal(sparkSeries(null), null);
    assert.equal(sparkSeries({ stepSeconds: 10, inBps: [], outBps: [] }), null);
    assert.equal(sparkSeries({ stepSeconds: 10, inBps: [5], outBps: [5, 6] }), null);
  });
  it("trims both series to the last ten minutes", () => {
    const series = sparkSeries({ stepSeconds: 300, inBps: [1, 2, 3, 4], outBps: [5, 6, 7, 8] });
    assert.deepEqual(series, { inBps: [2, 3, 4], outBps: [6, 7, 8] });
  });
});
