// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import "./boot.ts";
import { renderConsoleLinks, shownInActiveTab, wireConsoleClicks } from "./console-nav.ts";
import type { Bookmark, ConsoleInfo, RouterStatus } from "./contract.ts";
import { all, byId, cloneTemplate, setText } from "./dom.ts";
import { call, errorText, on } from "./ipc.ts";
import { displayUrl, hostOf } from "./lib/address.ts";
import { routerVersion } from "./lib/console-links.ts";
import { hopStates } from "./lib/router-view.ts";
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

function renderRouter(status: RouterStatus): void {
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
  byId("router-version").textContent = version ? `I2P ${version}` : "Unknown";
}

function renderConsole(info: ConsoleInfo): void {
  lastConsole = info;
  renderConsoleLinks(
    { list: byId("console-links"), note: byId("console-note"), title: byId("console-title") },
    info,
  );
  showVersion();
}

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
