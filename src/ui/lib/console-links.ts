// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import type { ConsoleInfo, RouterStatus } from "../contract.ts";

export const NO_CONSOLE_TEXT = "No router console found";

export const CONSOLE_LINK_TEXT = "I2P Router Console";

const TITLES: Record<NonNullable<ConsoleInfo["kind"]>, string> = {
  java: "Java I2P console",
  i2pd: "i2pd web console",
};

export interface ConsoleLinkView {
  found: boolean;
  note: string | null;
  title: string | null;
  label: string | null;
}

export function consoleLink(info: ConsoleInfo | null | undefined): ConsoleLinkView {
  if (!info?.found) return { found: false, note: NO_CONSOLE_TEXT, title: null, label: null };
  const title = info.kind ? TITLES[info.kind] : null;
  return { found: true, note: null, title, label: CONSOLE_LINK_TEXT };
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
