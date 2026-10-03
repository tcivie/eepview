// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import { renderConsoleLink, wireConsoleClicks } from "../console-nav.ts";
import type { ConsoleInfo, RouterStats, RouterStatus } from "../contract.ts";
import { all, byId } from "../dom.ts";
import { call, on } from "../ipc.ts";
import { routerVersion, shouldRedetect } from "../lib/console-links.ts";
import {
  type PanelText,
  panelControls,
  panelState,
  panelText,
  sparkSeries,
} from "../lib/router-panel.ts";
import { statsView } from "../lib/router-stats.ts";
import { areaPath, CHART_HEIGHT, linePath, scaleMax } from "../lib/sparkline.ts";
import { MISSING } from "../lib/stats-view.ts";
import { delegateClick } from "../shared/events.ts";
import { closeCurrent, report } from "./frame.ts";

const REFRESH_MS = 5000;
const EMPTY_PATH = `M0 ${CHART_HEIGHT}`;
const SPARK_PATHS = ["rp-spark-in", "rp-spark-area", "rp-spark-out"];
const quiet = (): undefined => undefined;
let refreshTimer = 0;
let lastStatus: RouterStatus | null = null;
let lastConsole: ConsoleInfo | null = null;

const STAT_FIELDS: Record<string, keyof PanelText> = {
  "rp-version": "version",
  "rp-uptime": "uptime",
  "rp-peers": "peers",
  "rp-build": "build",
  "rp-tunnels": "tunnels",
  "rp-bandwidth": "bandwidth",
};

const panel = (): HTMLElement => byId("router-panel");

function setTitle(el: HTMLElement, title: string | null): void {
  if (title) el.title = title;
  else el.removeAttribute("title");
}

function renderControls(status: RouterStatus): void {
  const controls = panelControls(status);
  byId("rp-pause").textContent = controls.pauseLabel;
  byId("rp-pause").dataset.command = controls.pauseCommand;
  for (const button of all<HTMLButtonElement>("[data-action]", panel())) {
    button.setAttribute("aria-disabled", String(!controls.routerEnabled));
    setTitle(button, controls.routerTitle);
  }
  byId("rp-managed-note").textContent = controls.routerTitle ?? "";
}

export function renderPanelStatus(status: RouterStatus): void {
  const state = panelState(status);
  byId("rp-head").dataset.tone = state.tone;
  byId("rp-title").textContent = state.label;
  byId("rp-text").textContent = state.text;
  byId("rp-proxy").textContent = status.proxy || MISSING;
  renderControls(status);
  const ready = shouldRedetect(lastStatus, status);
  lastStatus = status;
  if (ready && !panel().hidden) detectConsole();
}

/** Probes for a console, then loads the figures again: a console found now fills them. */
function detectConsole(): void {
  call("console_detect", {})
    .then((info) => {
      renderConsole(info);
      return refresh();
    })
    .catch(quiet);
}

function renderConsole(info: ConsoleInfo): void {
  lastConsole = info;
  renderConsoleLink(
    {
      list: byId("rp-console-links"),
      note: byId("rp-console-note"),
      title: byId("rp-console-title"),
    },
    info,
  );
  if (byId("rp-version").textContent === MISSING) showConsoleVersion();
}

function showConsoleVersion(): void {
  const version = routerVersion(lastStatus?.version ?? null, lastConsole);
  if (version) byId("rp-version").textContent = `I2P ${version}`;
}

function renderSpark(history: ReturnType<typeof statsView>["history"]): void {
  const series = sparkSeries(history);
  if (!series) {
    for (const id of SPARK_PATHS) byId(id).setAttribute("d", EMPTY_PATH);
    return;
  }
  const max = scaleMax([...series.inBps, ...series.outBps]);
  byId("rp-spark-in").setAttribute("d", linePath(series.inBps, max));
  byId("rp-spark-area").setAttribute("d", areaPath(series.inBps, max));
  byId("rp-spark-out").setAttribute("d", linePath(series.outBps, max));
}

function renderStats(stats: RouterStats): void {
  const view = statsView(stats);
  const text = panelText(view);
  for (const [id, key] of Object.entries(STAT_FIELDS)) byId(id).textContent = text[key];
  if (text.version === MISSING) showConsoleVersion();
  renderSpark(view.history);
}

/** Loads the figures, then reports the new size: the panel may have grown. */
async function refresh(): Promise<void> {
  await Promise.allSettled([
    call("router_status", {}).then(renderPanelStatus),
    call("router_stats", {}).then(renderStats),
  ]);
  report();
}

/** The panel opened: refresh it now and every 5 s while it is open. */
export function openPanel(): void {
  refresh().catch(quiet);
  detectConsole();
  window.clearInterval(refreshTimer);
  refreshTimer = window.setInterval(() => refresh().catch(quiet), REFRESH_MS);
  byId("rp-title").focus();
}

/** The panel closed: stop the refresh. */
export function stopPanel(): void {
  window.clearInterval(refreshTimer);
}

function afterCommand(command: Promise<unknown>): void {
  command.then(refresh).catch(quiet);
}

function onPause(): void {
  const resume = byId("rp-pause").dataset.command === "connection_resume";
  afterCommand(resume ? call("connection_resume", {}) : call("connection_pause", {}));
}

function onRouterAction(button: HTMLElement): void {
  if (button.getAttribute("aria-disabled") === "true") return;
  const action = button.dataset.action === "stop" ? "stop" : "restart";
  afterCommand(call("router_control", { action }));
}

function openNetworkPage(): void {
  const input = byId("rp-network").dataset.open ?? "";
  call("navigate", { input }).catch(quiet);
  closeCurrent(false);
}

export function wireRouterPanel(): void {
  byId("rp-pause").addEventListener("click", onPause);
  delegateClick<HTMLElement>(panel(), "[data-action]", onRouterAction);
  byId("rp-network").addEventListener("click", openNetworkPage);
  on("router-status", renderPanelStatus).catch(quiet);
  wireConsoleClicks(byId("rp-console-links"), () => closeCurrent(false));
  call("console_status", {}).then(renderConsole).catch(quiet);
  on("console-changed", renderConsole).catch(quiet);
}
