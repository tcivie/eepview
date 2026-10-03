// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import type { ConsoleInfo, ConsolePage, RouterStatus } from "../contract.ts";

export const NO_CONSOLE_TEXT = "No router console found";

export const CONSOLE_PAGE_ORDER: readonly ConsolePage[] = [
  "home",
  "tunnels",
  "addressbook",
  "config",
  "logs",
];

export const CONSOLE_PAGE_LABELS: Record<ConsolePage, string> = {
  home: "Console",
  tunnels: "Tunnels",
  addressbook: "Address book",
  config: "Config",
  logs: "Logs",
};

const TITLES: Record<NonNullable<ConsoleInfo["kind"]>, string> = {
  java: "Java I2P console",
  i2pd: "i2pd web console",
};

export interface ConsoleLinkView {
  page: ConsolePage;
  label: string;
}

export interface ConsoleLinksView {
  found: boolean;
  note: string | null;
  title: string | null;
  links: ConsoleLinkView[];
}

export function consoleLinks(info: ConsoleInfo | null | undefined): ConsoleLinksView {
  if (!info?.found) return { found: false, note: NO_CONSOLE_TEXT, title: null, links: [] };
  const links = CONSOLE_PAGE_ORDER.filter((page) => info.pages.includes(page)).map((page) => ({
    page,
    label: CONSOLE_PAGE_LABELS[page],
  }));
  return { found: true, note: null, title: info.kind ? TITLES[info.kind] : null, links };
}

export function routerVersion(
  statusVersion: string | null,
  info: ConsoleInfo | null | undefined,
): string | null {
  return statusVersion ?? (info?.found ? info.version : null) ?? null;
}

export function shouldRedetect(
  prev: Pick<RouterStatus, "state" | "paused"> | null,
  next: Pick<RouterStatus, "state" | "paused">,
): boolean {
  return next.state === "ok" && !next.paused && prev?.state !== "ok";
}
