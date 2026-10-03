// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import "./boot.ts";
import type { Bookmark, RouterStatus } from "./contract.ts";
import { all, byId, cloneTemplate, setText } from "./dom.ts";
import { call, errorText, on } from "./ipc.ts";
import { displayUrl, hostOf } from "./lib/address.ts";
import { CRASH_TEXT, reportHref } from "./lib/report-page.ts";
import { hopStates, versionText } from "./lib/router-view.ts";
import { renderRouterSummary } from "./shared/router-summary.ts";
import { renderSiteMark } from "./site-mark.ts";

const MAX_TILES = 11;
const quiet = (): undefined => undefined;

function tile(bookmark: Bookmark): HTMLElement {
  const item = cloneTemplate("tile-template");
  item.querySelector("a")?.setAttribute("href", bookmark.url);
  const mark = item.querySelector(".tile-mark");
  if (mark) renderSiteMark(mark, bookmark.icon, (hostOf(bookmark.url)[0] ?? "?").toUpperCase());
  setText(item, ".tile-name", bookmark.title || hostOf(bookmark.url));
  setText(item, ".tile-addr", displayUrl(bookmark.url));
  return item;
}

function renderTiles(bookmarks: Bookmark[]): void {
  const tiles = bookmarks.slice(0, MAX_TILES).map(tile);
  byId("tiles").replaceChildren(...tiles, cloneTemplate("tile-add-template"));
  byId("tiles-error").hidden = true;
}

function loadTiles(): void {
  call("bookmarks_list", {})
    .then(renderTiles)
    .catch((error) => {
      byId("tiles-error").hidden = false;
      byId("tiles-error").textContent = errorText(error);
    });
}

function renderRouter(status: RouterStatus): void {
  const view = renderRouterSummary(
    { chip: byId("router-chip"), text: byId("router-text"), proxy: byId("router-proxy") },
    status,
  );
  byId("router-version").textContent = versionText(status);
  const states = hopStates(view.tone, all("#router-hops .hop").length);
  all<HTMLElement>("#router-hops .hop").forEach((hop, i) => {
    const state = states[i];
    if (state) hop.dataset.state = state;
    else delete hop.dataset.state;
  });
}

function hideCrashBanner(): Promise<void> {
  byId("crash-banner").hidden = true;
  return call("diag_crash_dismiss", {}).catch(quiet);
}

function showCrashBanner(crashed: boolean): void {
  byId("crash-text").textContent = CRASH_TEXT;
  byId("crash-banner").hidden = !crashed;
}

byId("crash-dismiss").addEventListener("click", () => {
  hideCrashBanner().catch(quiet);
});
byId("crash-report").addEventListener("click", () => {
  hideCrashBanner().then(() => window.location.assign(reportHref("crash")));
});
call("diag_crash_status", {}).then(showCrashBanner).catch(quiet);
loadTiles();
call("router_status", {}).then(renderRouter).catch(quiet);
on("router-status", renderRouter).catch(quiet);
on("bookmarks-changed", loadTiles).catch(quiet);
on("icons-changed", loadTiles).catch(quiet);
