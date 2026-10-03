// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

export interface LinkHover {
  text: string;
  blocked: boolean;
}

export interface LinkStatusView {
  visible: boolean;
  prefix: string | null;
  text: string;
}

export const MAX_STATUS_CHARS = 512;
export const BLOCKED_PREFIX = "Blocked:";

export function cleanStatusText(text: string): string {
  const single = text.replace(/\s+/g, " ").trim();
  return single.length > MAX_STATUS_CHARS ? `${single.slice(0, MAX_STATUS_CHARS - 1)}…` : single;
}

export interface PillSize {
  width: number;
  height: number;
}

/** The size the shell gives the status webview: whole pixels, never cutting the pill. */
export function pillSize(rect: PillSize): PillSize {
  return { width: Math.ceil(rect.width), height: Math.ceil(rect.height) };
}

/** The shell sends `Blocked: <host>` for a refused link. The bubble draws the prefix itself. */
function withoutBlockedPrefix(text: string): string {
  return text.startsWith(BLOCKED_PREFIX) ? text.slice(BLOCKED_PREFIX.length).trim() : text;
}

export function linkStatus(hover: LinkHover | null): LinkStatusView {
  const text = cleanStatusText(hover?.text ?? "");
  if (text === "") return { visible: false, prefix: null, text: "" };
  if (hover?.blocked)
    return { visible: true, prefix: BLOCKED_PREFIX, text: withoutBlockedPrefix(text) };
  return { visible: true, prefix: null, text };
}
