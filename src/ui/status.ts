// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import "./boot.ts";
import { byId } from "./dom.ts";
import { devMode, on } from "./ipc.ts";
import { eepsiteUrl } from "./lib/address.ts";
import { type LinkHover, linkStatus } from "./lib/link-status.ts";

function show(hover: LinkHover | null): void {
  const view = linkStatus(hover);
  const bubble = byId("link-status");
  bubble.dataset.visible = String(view.visible);
  byId("link-blocked").hidden = view.prefix === null;
  if (view.visible) byId("link-text").textContent = view.text;
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
if (devMode) previewFromParams();
