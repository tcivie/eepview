import "./boot.ts";
import type { Bookmark, RouterStatus } from "./contract.ts";
import { all, byId, cloneTemplate, setText } from "./dom.ts";
import { call, errorText, on } from "./ipc.ts";
import { displayUrl, hostOf } from "./lib/address.ts";
import { hopStates, routerView } from "./lib/router-view.ts";

const MAX_TILES = 11;
const quiet = (): undefined => undefined;

function tile(bookmark: Bookmark): HTMLElement {
  const item = cloneTemplate("tile-template");
  item.querySelector("a")?.setAttribute("href", bookmark.url);
  setText(item, ".tile-mark", (hostOf(bookmark.url)[0] ?? "?").toUpperCase());
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
  const view = routerView(status);
  const chip = byId("router-chip");
  chip.dataset.tone = view.tone;
  chip.textContent = view.label;
  byId("router-text").textContent = view.text;
  byId("router-version").textContent = status.version ? `I2P ${status.version}` : "Unknown";
  byId("router-proxy").textContent = status.proxy || "None yet";
  const states = hopStates(view.tone, all("#router-hops .hop").length);
  all<HTMLElement>("#router-hops .hop").forEach((hop, i) => {
    const state = states[i];
    if (state) hop.dataset.state = state;
    else delete hop.dataset.state;
  });
}

loadTiles();
call("router_status", {}).then(renderRouter).catch(quiet);
on("router-status", renderRouter).catch(quiet);
on("bookmarks-changed", loadTiles).catch(quiet);
