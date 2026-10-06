// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// The load-failed page (docs/wiki/browser-shell.md, F7): what the query of the page says.

import { shortAddress } from "./address.ts";

export type FailReason = "blocked" | "unreachable" | "engine";

export interface LoadFailedView {
  url: string;
  address: string;
  reason: FailReason;
  reasonText: string;
  code: string;
}

const REASON_TEXT: Record<FailReason, string> = {
  blocked: "The system stopped the request before it left your computer, so nothing went out.",
  unreachable: "eepview could not open a connection to its own I2P proxy.",
  engine: "The web engine stopped the page with an error.",
};

function reasonOf(raw: string | null): FailReason {
  return raw === "blocked" || raw === "unreachable" ? raw : "engine";
}

/** Reads the page query. Unknown or missing values fall back to safe defaults. */
export function loadFailedView(search: string): LoadFailedView {
  const params = new URLSearchParams(search);
  const reason = reasonOf(params.get("reason"));
  const url = params.get("url") ?? "";
  return {
    url,
    address: shortAddress(url),
    reason,
    reasonText: REASON_TEXT[reason],
    code: params.get("code") ?? "",
  };
}
