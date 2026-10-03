// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import type { RouterStats } from "../contract.ts";

export const HISTORY_STEP_SECONDS = 5;

const MS_PER_SECOND = 1000;
const PERCENT = 100;

export interface StatsView {
  networkStatus: string | null;
  uptimeSeconds: number | null;
  uptimeResolutionSeconds: number | null;
  routerKind: string | null;
  routerVersion: string | null;
  javaVersion: string | null;
  bandwidthInBps: number | null;
  bandwidthOutBps: number | null;
  history: { stepSeconds: number; inBps: number[]; outBps: number[] } | null;
  clientTunnels: number | null;
  exploratoryTunnels: number | null;
  inboundTunnels: number | null;
  outboundTunnels: number | null;
  activePeers: number | null;
  participatingTunnels: number | null;
  buildSuccessRate: number | null;
  knownRouters: number | null;
  floodfills: number | null;
}

const seconds = (ms: number | null): number | null =>
  ms === null ? null : Math.floor(ms / MS_PER_SECOND);

const resolutionSeconds = (ms: number | null): number | null =>
  ms === null ? null : Math.max(1, Math.floor(ms / MS_PER_SECOND));

function buildRate(build: RouterStats["tunnelBuildSuccessPercent"]): number | null {
  const percent = build.total ?? build.exploratory;
  return percent === null ? null : percent / PERCENT;
}

function historyView(history: RouterStats["history"]): StatsView["history"] {
  if (history.length === 0) return null;
  return {
    stepSeconds: HISTORY_STEP_SECONDS,
    inBps: history.map((sample) => sample.in),
    outBps: history.map((sample) => sample.out),
  };
}

const NO_STATS: RouterStats = {
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
};

/** The view of a `router_stats` answer: the figures the router panel and the Network page show. */
export function statsView(answer: RouterStats | null | undefined): StatsView {
  const stats = answer ?? NO_STATS;
  return {
    networkStatus: stats.networkStatus,
    uptimeSeconds: seconds(stats.uptimeMs),
    uptimeResolutionSeconds: resolutionSeconds(stats.uptimeResolutionMs),
    routerKind: null,
    routerVersion: stats.version,
    javaVersion: null,
    bandwidthInBps: stats.bandwidthBytesPerSecond.in1s,
    bandwidthOutBps: stats.bandwidthBytesPerSecond.out1s,
    history: historyView(stats.history),
    clientTunnels: stats.tunnels.client,
    exploratoryTunnels: stats.tunnels.exploratory,
    inboundTunnels: stats.tunnels.in,
    outboundTunnels: stats.tunnels.out,
    activePeers: stats.activePeers,
    participatingTunnels: stats.tunnels.participating,
    buildSuccessRate: buildRate(stats.tunnelBuildSuccessPercent),
    knownRouters: stats.knownRouters,
    floodfills: stats.floodfills,
  };
}
