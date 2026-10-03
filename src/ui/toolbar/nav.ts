// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import type { RouterStatus, TabInfo, Toast } from "../contract.ts";
import { byId } from "../dom.ts";
import { call } from "../ipc.ts";
import { hostOf } from "../lib/address.ts";
import { jsToggleLabel } from "../lib/js-toggle.ts";
import { panelState } from "../lib/router-panel.ts";
import { routerView } from "../lib/router-view.ts";
import { bindClicks } from "../shared/events.ts";
import { closePopup, onPopupClosed, openKind, openPopup, wireToggle } from "./popups.ts";
import { activeTab } from "./state.ts";

const TOAST_MS = 4000;
const quiet = (): undefined => undefined;
let toastTimer = 0;

function setLabel(button: HTMLElement, labelId: string, text: string): void {
  button.title = text;
  byId(labelId).textContent = text;
}

function renderReload(loading: boolean): void {
  const button = byId("reload");
  button.dataset.mode = loading ? "stop" : "reload";
  setLabel(button, "reload-label", loading ? "Stop" : "Reload");
}

function renderStar(tab: TabInfo | undefined): void {
  const star = byId<HTMLButtonElement>("star");
  const saved = tab?.bookmarked ?? false;
  star.setAttribute("aria-pressed", String(saved));
  star.disabled = !tab || tab.kind === "internal";
  setLabel(star, "star-label", saved ? "Remove bookmark" : "Bookmark this page");
}

function renderJs(tab: TabInfo | undefined): void {
  const js = byId<HTMLButtonElement>("js");
  js.setAttribute("aria-pressed", String(tab?.jsOn ?? false));
  js.disabled = !tab || tab.kind === "internal";
  const label = jsToggleLabel(tab?.jsOn ?? false, tab ? hostOf(tab.url) : "");
  js.title = label;
  js.setAttribute("aria-label", label);
}

export function renderNav(tab: TabInfo | undefined): void {
  byId<HTMLButtonElement>("back").disabled = !tab?.canBack;
  byId<HTMLButtonElement>("forward").disabled = !tab?.canForward;
  renderReload(tab?.loading ?? false);
  renderStar(tab);
  renderJs(tab);
}

export function renderStatus(status: RouterStatus): void {
  const view = routerView(status);
  const state = panelState(status);
  byId("status").dataset.tone = state.tone;
  byId("status-label").textContent = `Router: ${status.paused ? state.label : view.label}`;
  byId("status-title").textContent = state.label;
  byId("status-text").textContent = status.paused ? state.text : view.text;
}

export function showToast(toast: Toast): void {
  const el = byId("toast");
  el.textContent = toast.text;
  el.dataset.kind = toast.kind;
  el.hidden = false;
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    el.hidden = true;
  }, TOAST_MS);
}

async function toggleBookmark(): Promise<void> {
  const tab = activeTab();
  if (!tab) return;
  const existing = await call("bookmark_find", { url: tab.url });
  if (existing) await call("bookmark_remove", { id: existing.id });
  else await call("bookmark_add", { bookmark: { url: tab.url, title: tab.title } });
}

function toggleJs(): void {
  const tab = activeTab();
  if (tab) call("site_js_set", { host: hostOf(tab.url), on: !tab.jsOn }).catch(quiet);
}

function reloadOrStop(): void {
  const stop = byId("reload").dataset.mode === "stop";
  (stop ? call("stop", {}) : call("reload", {})).catch(quiet);
}

/** The router hint: shown on hover, never over the menu or the router panel (rule 6, 7). */
function showHint(): void {
  const kind = openKind();
  if (kind === "menu" || kind === "router") return;
  const data = {
    title: byId("status-title").textContent ?? "",
    text: byId("status-text").textContent ?? "",
  };
  openPopup("hint", byId("status"), data);
}

function setExpanded(id: string, open: boolean): void {
  byId(id).setAttribute("aria-expanded", String(open));
}

function opener(kind: "menu" | "router", buttonId: string): void {
  const button = byId(buttonId);
  wireToggle(button, kind, () => {
    openPopup(kind, button);
    setExpanded(buttonId, true);
  });
  onPopupClosed(kind, (refocus) => {
    setExpanded(buttonId, false);
    if (refocus) button.focus();
  });
}

function wirePopupButtons(): void {
  const status = byId("status");
  status.addEventListener("mouseenter", showHint);
  status.addEventListener("mouseleave", () => closePopup("hint"));
  status.addEventListener("pointerdown", () => closePopup("hint"));
  opener("router", "status");
  opener("menu", "menu-btn");
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") closePopup("hint");
  });
}

export function wireNav(): void {
  bindClicks(
    {
      back: () => call("go_back", {}).catch(quiet),
      forward: () => call("go_forward", {}).catch(quiet),
      reload: reloadOrStop,
      home: () => call("home", {}).catch(quiet),
      star: () => toggleBookmark().catch(quiet),
      js: toggleJs,
    },
    byId,
  );
  wirePopupButtons();
}

export function openForReview(kind: "menu" | "router" | "hint"): void {
  if (kind === "hint") showHint();
  else openPopup(kind, byId(kind === "menu" ? "menu-btn" : "status"));
}
