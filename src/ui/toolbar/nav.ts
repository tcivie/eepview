import type { RouterStatus, TabInfo, Toast } from "../contract.ts";
import { byId } from "../dom.ts";
import { call } from "../ipc.ts";
import { hostOf } from "../lib/address.ts";
import { routerView } from "../lib/router-view.ts";
import { zoomText } from "../lib/tab-strip.ts";
import { bindClicks } from "../shared/events.ts";
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
  const host = tab ? hostOf(tab.url) : "";
  js.title = tab?.jsOn ? `JavaScript is on for ${host}` : `JavaScript is off for ${host}`;
}

export function renderNav(tab: TabInfo | undefined): void {
  byId<HTMLButtonElement>("back").disabled = !tab?.canBack;
  byId<HTMLButtonElement>("forward").disabled = !tab?.canForward;
  renderReload(tab?.loading ?? false);
  renderStar(tab);
  renderJs(tab);
  byId("zoom-value").textContent = zoomText(tab?.zoom ?? 1);
}

export function renderStatus(status: RouterStatus): void {
  const view = routerView(status);
  byId("status").dataset.tone = view.tone;
  byId("status-label").textContent = `Router: ${view.label}`;
  byId("status-title").textContent = view.title;
  byId("status-text").textContent = view.text;
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

function setTip(open: boolean): void {
  byId("status-tip").hidden = !open;
}

function setMenu(open: boolean): void {
  byId("menu").hidden = !open;
  byId("menu-btn").setAttribute("aria-expanded", String(open));
  if (open) byId("menu").querySelector<HTMLElement>("button")?.focus();
}

const MENU_COMMANDS: Record<string, () => Promise<unknown>> = {
  "new-tab": () => call("tab_new", {}),
  "zoom-in": () => call("zoom_in", {}),
  "zoom-out": () => call("zoom_out", {}),
  "zoom-reset": () => call("zoom_reset", {}),
};

function onMenuClick(event: MouseEvent): void {
  const item = (event.target as Element).closest<HTMLElement>("[data-open],[data-command]");
  if (!item) return;
  const command = MENU_COMMANDS[item.dataset.command ?? ""];
  const open = item.dataset.open;
  if (command) command().catch(quiet);
  if (open) call("navigate", { input: open }).catch(quiet);
  if (open || item.dataset.command === "new-tab") setMenu(false);
}

function wireStatus(): void {
  const status = byId("status");
  status.addEventListener("mouseenter", () => setTip(true));
  status.addEventListener("mouseleave", () => setTip(false));
  status.addEventListener("focus", () => setTip(true));
  status.addEventListener("blur", () => setTip(false));
  status.addEventListener("click", () => setTip(byId("status-tip").hidden === true));
}

function closePopups(): void {
  setMenu(false);
  setTip(false);
}

function closeMenuOutside(event: MouseEvent): void {
  if (!byId("menu").contains(event.target as Node)) setMenu(false);
}

function wireMenu(): void {
  byId("menu-btn").addEventListener("click", (e) => {
    e.stopPropagation();
    setMenu(byId("menu").hidden === true);
  });
  byId("menu").addEventListener("click", onMenuClick);
  document.addEventListener("click", closeMenuOutside);
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") closePopups();
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
  wireMenu();
  wireStatus();
}

export function openMenuForReview(): void {
  setMenu(true);
}
