// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import "./boot.ts";
import type { RouterStats, RouterStatus } from "./contract.ts";
import { byId } from "./dom.ts";
import { call, on } from "./ipc.ts";
import { areaPath, linePath, scaleMax } from "./lib/sparkline.ts";
import { formatRate, MISSING, statsText } from "./lib/stats-view.ts";
import { renderRouterSummary } from "./shared/router-summary.ts";

const REFRESH_MS = 5000;
const quiet = (): undefined => undefined;

const TEXT_TARGETS: Record<string, keyof ReturnType<typeof statsText>> = {
  "bw-in-now": "bandwidthIn",
  "bw-out-now": "bandwidthOut",
  "client-tunnels": "clientTunnels",
  "participating-tunnels": "participatingTunnels",
  "build-rate": "buildRate",
  "network-status": "networkStatus",
  uptime: "uptime",
  "router-kind": "router",
  "java-version": "java",
  "known-routers": "knownRouters",
  floodfills: "floodfills",
};

function clearChart(): void {
  for (const id of ["spark-in", "spark-in-area", "spark-out"]) byId(id).setAttribute("d", "M0 140");
  byId("spark-max").textContent = MISSING;
  byId("spark-summary").textContent = "No bandwidth figures yet.";
}

function renderChart(history: RouterStats["history"]): void {
  if (!history || history.inBps.length < 2) {
    clearChart();
    return;
  }
  const max = scaleMax([...history.inBps, ...history.outBps]);
  byId("spark-in").setAttribute("d", linePath(history.inBps, max));
  byId("spark-in-area").setAttribute("d", areaPath(history.inBps, max));
  byId("spark-out").setAttribute("d", linePath(history.outBps, max));
  byId("spark-max").textContent = formatRate(max);
  byId("spark-summary").textContent = "Bandwidth in and out over the last 10 minutes.";
}

function renderStats(stats: RouterStats): void {
  const view = statsText(stats);
  for (const [id, key] of Object.entries(TEXT_TARGETS)) byId(id).textContent = String(view[key]);
  byId("build-rate-bar").setAttribute("width", String(view.buildRateBar));
  renderChart(stats.history);
}

function renderRouter(status: RouterStatus): void {
  renderRouterSummary(
    { chip: byId("net-chip"), text: byId("router-detail"), proxy: byId("router-proxy") },
    status,
  );
}

function refresh(): void {
  call("router_stats", {}).then(renderStats).catch(quiet);
}

call("router_status", {}).then(renderRouter).catch(quiet);
on("router-status", renderRouter).catch(quiet);
refresh();
window.setInterval(refresh, REFRESH_MS);
