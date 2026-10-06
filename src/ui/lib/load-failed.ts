// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// The load-failed page (docs/wiki/browser-shell.md, F7): what the query of the page says.

export type FailReason = "blocked" | "unreachable" | "engine";

export interface LoadFailedView {
  address: string;
  reason: FailReason;
  reasonText: string;
  code: string;
}

const MAX_SHOWN = 2048;

const REASON_TEXT: Record<FailReason, string> = {
  blocked: "The system stopped the request before it left your computer, so nothing went out.",
  unreachable: "eepview could not open a connection to its own I2P proxy.",
  engine: "The web engine stopped the page with an error.",
};

function reasonOf(raw: string | null): FailReason {
  return raw === "blocked" || raw === "unreachable" ? raw : "engine";
}

function shortened(address: string): string {
  return address.length > MAX_SHOWN ? `${address.slice(0, MAX_SHOWN - 1)}…` : address;
}

/** Reads the page query. Unknown or missing values fall back to safe defaults. */
export function loadFailedView(search: string): LoadFailedView {
  const params = new URLSearchParams(search);
  const reason = reasonOf(params.get("reason"));
  return {
    address: shortened(params.get("url") ?? ""),
    reason,
    reasonText: REASON_TEXT[reason],
    code: params.get("code") ?? "",
  };
}
