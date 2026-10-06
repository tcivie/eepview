// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import "./boot.ts";
import { byId } from "./dom.ts";
import { call, devMode } from "./ipc.ts";
import { loadFailedView } from "./lib/load-failed.ts";
import { reportHref } from "./lib/report-page.ts";

// The gallery (?dev=1) shows a sample failure.
const SAMPLE = "?url=http%3A%2F%2Fstats.i2p%2F&reason=blocked&code=NSURLErrorDomain+-1022";

const query = new URLSearchParams(window.location.search);
const search = devMode && !query.has("url") ? SAMPLE : window.location.search;
const view = loadFailedView(search);
const address = new URLSearchParams(search).get("url") ?? "";

byId("failed-url").textContent = view.address;
byId("failed-reason").textContent = view.reasonText;
byId("failed-code").textContent = view.code;
byId("failed-code-line").hidden = view.code === "";
byId<HTMLAnchorElement>("report-link").href = reportHref("load-failed");
byId("back-btn").addEventListener("click", () => window.history.back());
const retry = byId<HTMLButtonElement>("retry-btn");
retry.disabled = address === "";
retry.addEventListener("click", () => {
  call("navigate", { input: address }).catch(() => undefined);
});
