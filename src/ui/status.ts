// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import "./boot.ts";
import { byId } from "./dom.ts";
import { devMode, on, tell } from "./ipc.ts";
import { eepsiteUrl } from "./lib/address.ts";
import { type LinkHover, linkStatus, pillSize } from "./lib/link-status.ts";

/** Measures the pill at its natural width and tells the shell, which fits the webview. */
function report(): void {
  const measure = byId("link-measure");
  measure.replaceChildren(...byId("link-status").cloneNode(true).childNodes);
  const rect = measure.getBoundingClientRect();
  tell("status-size", pillSize(rect)).catch(() => undefined);
}

function show(hover: LinkHover | null): void {
  const view = linkStatus(hover);
  const bubble = byId("link-status");
  bubble.dataset.visible = String(view.visible);
  byId("link-blocked").hidden = view.prefix === null;
  if (!view.visible) return;
  byId("link-text").textContent = view.text;
  report();
}

function previewFromParams(): void {
  const params = new URLSearchParams(window.location.search);
  document.body.classList.add("status-preview");
  show({
    text: params.get("text") ?? eepsiteUrl("stats.i2p/cgi-bin/newhosts.txt"),
    blocked: params.has("blocked"),
  });
}

on("link-hover", show).catch(() => undefined);
on("status-side", (side) => {
  byId("link-status").dataset.side = side;
}).catch(() => undefined);
if (devMode) previewFromParams();
