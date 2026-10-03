import { type RouterLike, type RouterTone, routerView } from "./router-view.ts";
import { formatCount, formatPercent, formatRate, formatUptime, MISSING } from "./stats-view.ts";

export const SPARK_WINDOW_SECONDS = 600;
export const UNMANAGED_TITLE = "Only for a router that eepview manages";
const MIN_POINTS = 2;

export interface PanelStatsLike {
  routerVersion: string | null;
  uptimeSeconds: number | null;
  activePeers: number | null;
  inboundTunnels: number | null;
  outboundTunnels: number | null;
  participatingTunnels: number | null;
  buildSuccessRate: number | null;
  bandwidthInBps: number | null;
  bandwidthOutBps: number | null;
}

export interface PanelText {
  version: string;
  uptime: string;
  peers: string;
  tunnels: string;
  build: string;
  bandwidth: string;
}

export interface PanelStatusLike extends RouterLike {
  managed: boolean;
  paused: boolean;
}

export interface PanelState {
  tone: RouterTone;
  label: string;
  text: string;
}

export interface PanelControls {
  pauseLabel: string;
  pauseCommand: "connection_pause" | "connection_resume";
  routerEnabled: boolean;
  routerTitle: string | null;
}

export function tunnelsText(
  inbound: number | null,
  outbound: number | null,
  part: number | null,
): string {
  return `${formatCount(inbound)} in · ${formatCount(outbound)} out · ${formatCount(part)} participating`;
}

export function bandwidthText(inBps: number | null, outBps: number | null): string {
  return `${formatRate(inBps)} in · ${formatRate(outBps)} out`;
}

export function panelText(stats: PanelStatsLike): PanelText {
  return {
    version: stats.routerVersion ? `I2P ${stats.routerVersion}` : MISSING,
    uptime: formatUptime(stats.uptimeSeconds),
    peers: formatCount(stats.activePeers),
    tunnels: tunnelsText(stats.inboundTunnels, stats.outboundTunnels, stats.participatingTunnels),
    build: formatPercent(stats.buildSuccessRate),
    bandwidth: bandwidthText(stats.bandwidthInBps, stats.bandwidthOutBps),
  };
}

export function panelState(status: PanelStatusLike): PanelState {
  if (status.paused) {
    return { tone: "stopped", label: "Paused", text: "eepview opens no sites until you resume." };
  }
  const view = routerView({ ...status, version: null });
  return { tone: view.tone, label: view.title, text: view.text };
}

export function panelControls(status: PanelStatusLike): PanelControls {
  return {
    pauseLabel: status.paused ? "Resume I2P browsing" : "Pause I2P browsing",
    pauseCommand: status.paused ? "connection_resume" : "connection_pause",
    routerEnabled: status.managed,
    routerTitle: status.managed ? null : UNMANAGED_TITLE,
  };
}

export function recentWindow(values: readonly number[], stepSeconds: number): number[] {
  if (stepSeconds <= 0) return [...values];
  const count = Math.max(MIN_POINTS, Math.ceil(SPARK_WINDOW_SECONDS / stepSeconds) + 1);
  return values.slice(-count);
}

export interface SparkHistory {
  stepSeconds: number;
  inBps: readonly number[];
  outBps: readonly number[];
}

export function sparkSeries(
  history: SparkHistory | null,
): { inBps: number[]; outBps: number[] } | null {
  if (!history) return null;
  const inBps = recentWindow(history.inBps, history.stepSeconds);
  const outBps = recentWindow(history.outBps, history.stepSeconds);
  return inBps.length < MIN_POINTS || outBps.length < MIN_POINTS ? null : { inBps, outBps };
}
