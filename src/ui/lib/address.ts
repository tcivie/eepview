// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import type { TabInfo } from "../contract.ts";

const I2P_HOST = /^(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+i2p$/;
const WEB_SCHEME = /^https?:/i;
const INTERNAL_SCHEME = "eepview:";
const EEPSITE_SCHEME = "http:";
const ANY_SCHEME = /^[a-z][a-z0-9+.-]*:/i;

export function hostOf(input: string): string {
  const bare = input.trim().toLowerCase().replace(WEB_SCHEME, "");
  const authority = bare.replace(/^\/+/, "").split(/[/?#]/)[0] ?? "";
  return authority.replace(/^[^@]*@/, "").replace(/:\d*$/, "");
}

export interface AddressBadge {
  text: string;
  title: string;
  kind: "i2p" | "console";
}

const I2P_BADGE: AddressBadge = { text: "I2P", title: "Opened over I2P", kind: "i2p" };
const CONSOLE_BADGE: AddressBadge = {
  text: "Router console",
  title: "The router's own console on this computer",
  kind: "console",
};

/** The badge before the address: I2P for a web tab, Router console for the console tab. */
export function addressBadge(
  tab: Pick<TabInfo, "kind" | "url"> | null | undefined,
): AddressBadge | null {
  if (!tab) return null;
  if (tab.kind === "console") return CONSOLE_BADGE;
  return tab.kind === "web" && !isInternal(tab.url) ? I2P_BADGE : null;
}

export function isI2pAddress(input: string): boolean {
  return I2P_HOST.test(hostOf(input));
}

export function isInternal(url: string): boolean {
  return url.trim().toLowerCase().startsWith(INTERNAL_SCHEME);
}

export function eepsiteUrl(input: string): string {
  const trimmed = input.trim();
  const withScheme = WEB_SCHEME.test(trimmed) ? trimmed : `${EEPSITE_SCHEME}${trimmed}`;
  return new URL(withScheme).href;
}

export function displayUrl(url: string): string {
  if (isInternal(url)) return url;
  const withoutScheme = url.replace(WEB_SCHEME, "").replace(/^\/+/, "");
  return withoutScheme.endsWith("/") && withoutScheme.indexOf("/") === withoutScheme.length - 1
    ? withoutScheme.slice(0, -1)
    : withoutScheme;
}

export function internalPageOf(url: string): string | null {
  if (!isInternal(url)) return null;
  const rest = url.trim().slice(INTERNAL_SCHEME.length).replace(/^\/+/, "");
  return rest.split(/[/?#]/)[0] || null;
}

export function internalUrlForFile(href: string): string | null {
  if (ANY_SCHEME.test(href)) return null;
  const match = /(?:^|\/)([a-z-]+)\.html(\?[^#]*)?(?:#.*)?$/.exec(href);
  if (!match) return null;
  return `eepview://${match[1]}${match[2] ?? ""}`;
}
