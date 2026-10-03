// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import "./boot.ts";
import { renderConsoleLink, shownInActiveTab, wireConsoleClicks } from "./console-nav.ts";
import type { Bookmark, ConsoleInfo, RouterStatus } from "./contract.ts";
import { all, byId, cloneTemplate, setText } from "./dom.ts";
import { call, errorText, on } from "./ipc.ts";
import { displayUrl, hostOf } from "./lib/address.ts";
import { routerVersion, shouldRedetect } from "./lib/console-links.ts";
import { CRASH_TEXT, reportHref } from "./lib/report-page.ts";
import { hopStates, versionText } from "./lib/router-view.ts";
import { renderRouterSummary } from "./shared/router-summary.ts";
import { renderSiteMark } from "./site-mark.ts";

const MAX_TILES = 11;
const quiet = (): undefined => undefined;
let lastStatus: RouterStatus | null = null;
let lastConsole: ConsoleInfo | null = null;

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

function redetectWhenReady(status: RouterStatus): void {
  if (!shouldRedetect(lastStatus, status)) return;
  shownInActiveTab("home")
    .then((shown) => (shown ? call("console_detect", {}).then(renderConsole) : undefined))
    .catch(quiet);
}

function renderRouter(status: RouterStatus): void {
  redetectWhenReady(status);
  const view = renderRouterSummary(
    { chip: byId("router-chip"), text: byId("router-text"), proxy: byId("router-proxy") },
    status,
  );
  lastStatus = status;
  showVersion();
  const states = hopStates(view.tone, all("#router-hops .hop").length);
  all<HTMLElement>("#router-hops .hop").forEach((hop, i) => {
    const state = states[i];
    if (state) hop.dataset.state = state;
    else delete hop.dataset.state;
  });
}

function showVersion(): void {
  const version = routerVersion(lastStatus?.version ?? null, lastConsole);
  byId("router-version").textContent = versionText({ version });
}

function renderConsole(info: ConsoleInfo): void {
  lastConsole = info;
  renderConsoleLink(
    { list: byId("console-links"), note: byId("console-note"), title: byId("console-title") },
    info,
  );
  showVersion();
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
wireConsoleClicks(byId("console-links"));
shownInActiveTab("home")
  .catch(() => false)
  .then((shown) => call(shown ? "console_detect" : "console_status", {}))
  .then(renderConsole)
  .catch(quiet);
on("console-changed", renderConsole).catch(quiet);
call("router_status", {}).then(renderRouter).catch(quiet);
on("router-status", renderRouter).catch(quiet);
on("bookmarks-changed", loadTiles).catch(quiet);
on("icons-changed", loadTiles).catch(quiet);
