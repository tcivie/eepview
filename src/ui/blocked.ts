// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import "./boot.ts";
import { announce, byId } from "./dom.ts";
import { reportHref } from "./lib/report-page.ts";

const SAMPLE_REFUSED = "https://www.example.com/account/login?next=%2Fsettings&lang=en";
const MAX_SHOWN = 2048;

function refusedAddress(): string {
  const raw = new URLSearchParams(window.location.search).get("url") ?? SAMPLE_REFUSED;
  return raw.length > MAX_SHOWN ? `${raw.slice(0, MAX_SHOWN)}…` : raw;
}

async function copyAddress(address: string): Promise<void> {
  const status = byId("blocked-status");
  try {
    await navigator.clipboard.writeText(address);
    announce(status, "Address copied.");
  } catch {
    announce(status, "Could not copy. Select the address and copy it by hand.");
  }
}

const address = refusedAddress();
byId("refused-url").textContent = address;
byId<HTMLAnchorElement>("report-link").href = reportHref("blocked");
byId("back-btn").addEventListener("click", () => window.history.back());
byId("copy-btn").addEventListener("click", () => {
  copyAddress(address).catch(() => undefined);
});
