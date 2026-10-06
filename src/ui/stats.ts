// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import "./boot.ts";
import type { RouterStats, RouterStatus } from "./contract.ts";
import { byId } from "./dom.ts";
import { call, on } from "./ipc.ts";
import { sparkSeries } from "./lib/router-panel.ts";
import { statsView } from "./lib/router-stats.ts";
import { areaPath, CHART_HEIGHT, linePath, scaleMax } from "./lib/sparkline.ts";
import { formatRate, MISSING, statsText } from "./lib/stats-view.ts";
import { renderRouterSummary } from "./shared/router-summary.ts";

const REFRESH_MS = 5000;
const EMPTY_PATH = `M0 ${CHART_HEIGHT}`;
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
  for (const id of ["spark-in", "spark-in-area", "spark-out"])
    byId(id).setAttribute("d", EMPTY_PATH);
  byId("spark-max").textContent = MISSING;
  byId("spark-summary").textContent = "No bandwidth figures yet.";
}

function renderChart(history: ReturnType<typeof statsView>["history"]): void {
  const series = sparkSeries(history);
  if (!series) {
    clearChart();
    return;
  }
  const max = scaleMax([...series.inBps, ...series.outBps]);
  byId("spark-in").setAttribute("d", linePath(series.inBps, max) || EMPTY_PATH);
  byId("spark-in-area").setAttribute("d", areaPath(series.inBps, max) || EMPTY_PATH);
  byId("spark-out").setAttribute("d", linePath(series.outBps, max) || EMPTY_PATH);
  byId("spark-max").textContent = formatRate(max);
  byId("spark-summary").textContent = "Bandwidth in and out over the last 10 minutes.";
}

function renderStats(stats: RouterStats): void {
  const figures = statsView(stats, Date.now());
  const view = statsText(figures);
  for (const [id, key] of Object.entries(TEXT_TARGETS)) byId(id).textContent = String(view[key]);
  byId("build-rate-bar").setAttribute("width", String(view.buildRateBar));
  renderChart(figures.history);
}

function renderRouter(status: RouterStatus): void {
  renderRouterSummary(
    { chip: byId("net-chip"), text: byId("router-detail"), proxy: byId("router-proxy") },
    status,
  );
}

/** Loads the figures while the page shows: a hidden page asks the router nothing. */
function refresh(): void {
  if (document.visibilityState !== "visible") return;
  call("router_stats", {}).then(renderStats).catch(quiet);
}

call("router_status", {}).then(renderRouter).catch(quiet);
on("router-status", renderRouter).catch(quiet);
refresh();
call("console_detect", {}).then(refresh).catch(quiet);
document.addEventListener("visibilitychange", refresh);
window.setInterval(refresh, REFRESH_MS);
