export const MISSING = "—";

export interface StatsLike {
  networkStatus: string | null;
  uptimeSeconds: number | null;
  routerKind: string | null;
  routerVersion: string | null;
  javaVersion: string | null;
  bandwidthInBps: number | null;
  bandwidthOutBps: number | null;
  bandwidthHistory: { inBps: number[]; outBps: number[] } | null;
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

const BYTES_PER_KIB = 1024;
const MINUTE_S = 60;
const HOUR_S = 3600;
const DAY_S = 86_400;

export function formatRate(bps: number | null): string {
  if (bps === null) return MISSING;
  const kb = bps / BYTES_PER_KIB;
  return kb >= BYTES_PER_KIB ? `${(kb / BYTES_PER_KIB).toFixed(1)} MB/s` : `${kb.toFixed(1)} KB/s`;
}

export function formatCount(n: number | null): string {
  return n === null ? MISSING : n.toLocaleString("en");
}

export function formatPercent(rate: number | null): string {
  return rate === null ? MISSING : `${Math.round(rate * 100)}%`;
}

export function formatUptime(seconds: number | null): string {
  if (seconds === null) return MISSING;
  const days = Math.floor(seconds / DAY_S);
  const hours = Math.floor((seconds % DAY_S) / HOUR_S);
  const minutes = Math.floor((seconds % HOUR_S) / MINUTE_S);
  if (days > 0) return `${days} d ${hours} h`;
  if (hours > 0) return `${hours} h ${minutes} min`;
  return `${minutes} min`;
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
    uptime: formatUptime(stats.uptimeSeconds),
    router: formatRouter(stats.routerKind, stats.routerVersion),
    java: text(stats.javaVersion),
    knownRouters: formatCount(stats.knownRouters),
    floodfills: formatCount(stats.floodfills),
  };
}
