// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import type { CheckId, CheckState, VerifyCheck } from "../contract.ts";

const ORDER: readonly CheckId[] = ["proxy-i2p", "version", "no-outproxy", "tunnels"];

export const CHECK_NAMES: Record<CheckId, string> = {
  "proxy-i2p": "Proxy is an I2P router",
  version: "Router version is supported",
  "no-outproxy": "No outproxy",
  tunnels: "Network up, client tunnel built",
};

const STATE_TEXT: Record<CheckState, string> = {
  pending: "Waiting",
  running: "Checking",
  passed: "Passed",
  failed: "Failed",
  "not-checked": "Not checked",
};

export interface CheckView {
  id: CheckId;
  name: string;
  state: CheckState;
  stateText: string;
  detail: string | null;
}

const two = (n: number): string => String(n).padStart(2, "0");

export function clockTime(ms: number): string {
  const at = new Date(ms);
  return `${two(at.getHours())}:${two(at.getMinutes())}:${two(at.getSeconds())}`;
}

function knownState(state: CheckState): CheckState {
  return state in STATE_TEXT ? state : "pending";
}

function stateText(state: CheckState, passedAt: number | null): string {
  if (state === "passed" && typeof passedAt === "number") {
    return `Passed at ${clockTime(passedAt)}`;
  }
  return STATE_TEXT[state];
}

function checkOf(checks: VerifyCheck[] | null | undefined, id: CheckId): VerifyCheck {
  const found = (checks ?? []).find((c) => c.id === id);
  return found ?? { id, state: "pending", detail: null, passedAt: null };
}

function viewOf(check: VerifyCheck): CheckView {
  const state = knownState(check.state);
  return {
    id: check.id,
    name: CHECK_NAMES[check.id],
    state,
    stateText: stateText(state, check.passedAt),
    detail: check.detail || null,
  };
}

export function checkViews(checks: VerifyCheck[] | null | undefined): CheckView[] {
  return ORDER.map((id) => viewOf(checkOf(checks, id)));
}

export function checkAnnouncement(
  before: VerifyCheck[] | null | undefined,
  after: VerifyCheck[] | null | undefined,
): string {
  if (!before) return "";
  const old = checkViews(before);
  return checkViews(after)
    .filter((view, i) => view.state !== old[i]?.state)
    .map((view) => `${view.name}: ${view.stateText}.`)
    .join(" ");
}
