// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

export type RouterTone = "ready" | "building" | "stopped";

export interface RouterLike {
  state: "verifying" | "ok" | "building" | "down" | "not-i2p" | "outproxy";
  proxy: string;
  version: string | null;
  detail: string | null;
}

export interface RouterView {
  tone: RouterTone;
  label: string;
  title: string;
  text: string;
  canBrowse: boolean;
}

const VIEWS: Record<RouterLike["state"], Omit<RouterView, "text"> & { base: string }> = {
  verifying: {
    tone: "building",
    label: "Checking",
    title: "Checking the router",
    base: "eepview is making sure the proxy is a real I2P router.",
    canBrowse: false,
  },
  ok: {
    tone: "ready",
    label: "Ready",
    title: "Router ready",
    base: "Eepsites open through your I2P router.",
    canBrowse: true,
  },
  building: {
    tone: "building",
    label: "Building",
    title: "Building tunnels",
    base: "The first start takes 2 to 10 minutes. Sites open when tunnels are up.",
    canBrowse: false,
  },
  down: {
    tone: "stopped",
    label: "Stopped",
    title: "Router stopped",
    base: "Eepsites cannot load until the router runs again.",
    canBrowse: false,
  },
  "not-i2p": {
    tone: "stopped",
    label: "Not I2P",
    title: "The proxy is not an I2P router",
    base: "eepview refuses to send anything through it.",
    canBrowse: false,
  },
  outproxy: {
    tone: "stopped",
    label: "Outproxy",
    title: "The router has an outproxy",
    base: "It could reach the clearnet, so eepview does not use it.",
    canBrowse: false,
  },
};

export function routerView(status: RouterLike): RouterView {
  const { base, ...view } = VIEWS[status.state];
  const parts = [status.detail ?? base];
  if (status.version) parts.push(`I2P ${status.version} at ${status.proxy}.`);
  return { ...view, text: parts.join(" ") };
}

export type HopState = "built" | "building" | "refused" | null;

export function hopStates(tone: RouterTone, count: number): HopState[] {
  return Array.from({ length: count }, (_, i): HopState => {
    if (tone === "ready" || i === 0) return "built";
    if (tone === "stopped") return i === 1 ? "refused" : null;
    return i === 1 ? "building" : null;
  });
}

/** The router version for a page: `I2P <version>`, or why there is none. */
export function versionText(_status: { version: string | null }): string {
  return "";
}
