// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

export const MISSING = "—";

export interface StatsLike {
  networkStatus: string | null;
  uptimeSeconds: number | null;
  uptimeResolutionSeconds?: number | null;
  routerKind: string | null;
  routerVersion: string | null;
  javaVersion: string | null;
  bandwidthInBps: number | null;
  bandwidthOutBps: number | null;
  history: { stepSeconds: number; inBps: (number | null)[]; outBps: (number | null)[] } | null;
  clientTunnels: number | null;
  participatingTunnels: number | null;
  buildSuccessRate: number | null;
  knownRouters: number | null;
  floodfills: number | null;
}

export interface StatsText {
  bandwidthIn: string;
  bandwidthOut: string;
  clientTunnels: string;
  participatingTunnels: string;
  buildRate: string;
  buildRateBar: number;
  networkStatus: string;
  uptime: string;
  router: string;
  java: string;
  knownRouters: string;
  floodfills: string;
}

/** K = 1 000, as the Java I2P console writes its rates. */
const KILO = 1000;
const MEGA = KILO * KILO;
const TWO_DECIMALS_BELOW = 100;
const MINUTE_S = 60;
const HOUR_S = 3600;
const DAY_S = 86_400;

/** Decimals of a scaled rate, as the console writes them: 2 below 100, 1 below 1 000. */
function rateDecimals(value: number): number {
  if (value < TWO_DECIMALS_BELOW) return 2;
  return value < KILO ? 1 : 0;
}

/** `value` written with the decimals of its form, which come from the rounded value. */
function scaled(value: number): { text: string; rounded: number } {
  const rounded = Number(value.toFixed(rateDecimals(value)));
  return { text: value.toFixed(rateDecimals(rounded)), rounded };
}

/** A rate in bytes per second, with K = 1 000: "230 B/s", "53.91 kB/s", "2.50 MB/s". */
export function formatRate(bps: number | null): string {
  if (bps === null) return MISSING;
  if (Math.round(bps) < KILO) return `${Math.round(bps)} B/s`;
  const kilo = scaled(bps / KILO);
  if (kilo.rounded < KILO) return `${kilo.text} kB/s`;
  return `${scaled(bps / MEGA).text} MB/s`;
}

export function formatCount(n: number | null): string {
  return n === null ? MISSING : n.toLocaleString("en");
}

export function formatPercent(rate: number | null): string {
  return rate === null ? MISSING : `${(rate * 100).toFixed(1)}%`;
}

interface UptimeUnit {
  label: string;
  size: number;
  wrap: number;
}

const UPTIME_UNITS: readonly UptimeUnit[] = [
  { label: "d", size: DAY_S, wrap: Number.POSITIVE_INFINITY },
  { label: "h", size: HOUR_S, wrap: DAY_S },
  { label: "min", size: MINUTE_S, wrap: HOUR_S },
];

const unitCount = (unit: UptimeUnit, seconds: number): number =>
  Math.floor((seconds % unit.wrap) / unit.size);

/** The units the resolution allows: none is smaller than the resolution, days at least. */
function keptUnits(resolutionSeconds: number | null | undefined): readonly UptimeUnit[] {
  const kept = UPTIME_UNITS.filter((unit) => unit.size >= (resolutionSeconds ?? 0));
  return kept.length > 0 ? kept : UPTIME_UNITS.slice(0, 1);
}

/** The largest unit with a value and the next smaller one, without a unit under the resolution. */
export function formatUptime(seconds: number | null, resolutionSeconds?: number | null): string {
  if (seconds === null) return MISSING;
  const kept = keptUnits(resolutionSeconds);
  const found = UPTIME_UNITS.findIndex((unit) => unitCount(unit, seconds) > 0);
  const lead = found === -1 ? UPTIME_UNITS.length - 1 : found;
  const parts = UPTIME_UNITS.slice(lead, lead + 2).filter((unit) => kept.includes(unit));
  const shown = parts.length > 0 ? parts : kept.slice(-1);
  return shown.map((unit) => `${unitCount(unit, seconds)} ${unit.label}`).join(" ");
}

export function formatRouter(kind: string | null, version: string | null): string {
  const parts = [version ? `I2P ${version}` : null, kind].filter((p): p is string => !!p);
  return parts.length === 0 ? MISSING : parts.join(", ");
}

const text = (value: string | null): string => value ?? MISSING;

export function statsText(stats: StatsLike): StatsText {
  const rate = stats.buildSuccessRate;
  return {
    bandwidthIn: formatRate(stats.bandwidthInBps),
    bandwidthOut: formatRate(stats.bandwidthOutBps),
    clientTunnels: formatCount(stats.clientTunnels),
    participatingTunnels: formatCount(stats.participatingTunnels),
    buildRate: formatPercent(rate),
    buildRateBar: rate === null ? 0 : Math.round(Math.min(1, Math.max(0, rate)) * 100),
    networkStatus: text(stats.networkStatus),
    uptime: formatUptime(stats.uptimeSeconds, stats.uptimeResolutionSeconds),
    router: formatRouter(stats.routerKind, stats.routerVersion),
    java: text(stats.javaVersion),
    knownRouters: formatCount(stats.knownRouters),
    floodfills: formatCount(stats.floodfills),
  };
}
