import type { RouterStats, RouterStatus } from "../contract.ts";
import { all, byId } from "../dom.ts";
import { call } from "../ipc.ts";
import {
  type PanelText,
  panelControls,
  panelState,
  panelText,
  sparkSeries,
} from "../lib/router-panel.ts";
import { areaPath, CHART_HEIGHT, linePath, scaleMax } from "../lib/sparkline.ts";
import { MISSING } from "../lib/stats-view.ts";
import { delegateClick } from "../shared/events.ts";

const REFRESH_MS = 5000;
const EMPTY_PATH = `M0 ${CHART_HEIGHT}`;
const SPARK_PATHS = ["rp-spark-in", "rp-spark-area", "rp-spark-out"];
const quiet = (): undefined => undefined;
let refreshTimer = 0;

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
}

function renderSpark(history: RouterStats["history"]): void {
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
  const text = panelText(stats);
  for (const [id, key] of Object.entries(STAT_FIELDS)) byId(id).textContent = text[key];
  renderSpark(stats.history);
}

async function refresh(): Promise<void> {
  await Promise.allSettled([
    call("router_status", {}).then(renderPanelStatus),
    call("router_stats", {}).then(renderStats),
  ]);
}

function openPanel(): void {
  panel().hidden = false;
  byId("status").setAttribute("aria-expanded", "true");
  byId("status-tip").hidden = true;
  refresh().catch(quiet);
  window.clearInterval(refreshTimer);
  refreshTimer = window.setInterval(() => refresh().catch(quiet), REFRESH_MS);
  byId("rp-title").focus();
}

export function closePanel(returnFocus: boolean): void {
  if (panel().hidden) return;
  panel().hidden = true;
  window.clearInterval(refreshTimer);
  byId("status").setAttribute("aria-expanded", "false");
  if (returnFocus) byId("status").focus();
}

function togglePanel(): void {
  if (panel().hidden) openPanel();
  else closePanel(false);
}

function closeOutside(event: MouseEvent): void {
  const target = event.target as Node;
  if (panel().contains(target) || byId("status").contains(target)) return;
  closePanel(false);
}

function closeOnEscape(event: KeyboardEvent): void {
  if (event.key === "Escape" && !panel().hidden) closePanel(true);
}

function closeOnFocusLeave(event: FocusEvent): void {
  const next = event.relatedTarget as Node | null;
  if (next && !panel().contains(next) && !byId("status").contains(next)) closePanel(false);
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
  closePanel(false);
}

export function openRouterPanelForReview(): void {
  openPanel();
}

export function wireRouterPanel(): void {
  byId("status").addEventListener("click", togglePanel);
  document.addEventListener("click", closeOutside);
  document.addEventListener("keydown", closeOnEscape);
  panel().addEventListener("focusout", closeOnFocusLeave);
  byId("rp-pause").addEventListener("click", onPause);
  delegateClick<HTMLElement>(panel(), "[data-action]", onRouterAction);
  byId("rp-network").addEventListener("click", openNetworkPage);
}
