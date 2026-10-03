import "./boot.ts";
import type { RouterState, RouterStatus, TabInfo } from "./contract.ts";
import { byId } from "./dom.ts";
import { call, devMode, on } from "./ipc.ts";
import { focusAddress, previewSuggestions, showUrl, wireAddress } from "./toolbar/address.ts";
import { closeFind, openFind, showFindResult, wireFind } from "./toolbar/find.ts";
import { wireLayout } from "./toolbar/layout.ts";
import { openMenuForReview, renderNav, renderStatus, showToast, wireNav } from "./toolbar/nav.ts";
import {
  openRouterPanelForReview,
  renderPanelStatus,
  wireRouterPanel,
} from "./toolbar/router-panel.ts";
import { activeTab, onTabs, setTabs, updateTab } from "./toolbar/state.ts";
import { renderTabs, wireTabs } from "./toolbar/tabs.ts";

const quiet = (): undefined => undefined;

const SHORTCUTS: Record<string, () => void> = {
  "focus-address": focusAddress,
  "open-find": openFind,
  "close-find": closeFind,
};

function render(tabs: TabInfo[]): void {
  renderTabs(tabs);
  const tab = activeTab();
  renderNav(tab);
  showUrl(tab);
  byId("dev-frame-title").textContent = tab?.url ?? "";
}

function showRouter(status: RouterStatus): void {
  renderStatus(status);
  renderPanelStatus(status);
}

function listenToCore(): void {
  on("tabs-changed", setTabs).catch(quiet);
  on("tab-updated", updateTab).catch(quiet);
  on("find-result", showFindResult).catch(quiet);
  on("router-status", showRouter).catch(quiet);
  on("toast", showToast).catch(quiet);
  on("shortcut", ({ action }) => SHORTCUTS[action]?.()).catch(quiet);
}

function loadInitialState(): void {
  call("tab_list", {}).then(setTabs).catch(quiet);
  call("router_status", {}).then(showRouter).catch(quiet);
}

function previewRouter(state: string): void {
  call("router_status", {})
    .then((status) => showRouter({ ...status, state: state as RouterState }))
    .catch(quiet);
}

function previewFind(query: string): void {
  openFind();
  byId<HTMLInputElement>("find-input").value = query;
  byId("find-input").dispatchEvent(new Event("input"));
}

function applyReviewParams(params: URLSearchParams): void {
  if (params.has("find")) previewFind(params.get("find") ?? "");
  if (params.has("suggest")) previewSuggestions(params.get("suggest") ?? "");
  if (params.has("menu")) openMenuForReview();
  if (params.has("tip")) byId("status-tip").hidden = false;
  if (params.has("panel")) openRouterPanelForReview();
}

function showDevToast(): void {
  showToast({ kind: "info", text: "Downloads are not supported yet." });
}

function wireDevStage(): void {
  const params = new URLSearchParams(window.location.search);
  byId("dev-stage").hidden = false;
  const router = byId<HTMLSelectElement>("dev-router");
  router.value = params.get("router") ?? "ok";
  router.addEventListener("change", () => previewRouter(router.value));
  byId("dev-find").addEventListener("click", openFind);
  byId("dev-toast").addEventListener("click", showDevToast);
  window.setTimeout(() => applyReviewParams(params), 0);
}

onTabs(render);
wireTabs();
wireAddress();
wireFind();
wireNav();
wireLayout();
wireRouterPanel();
listenToCore();
loadInitialState();
if (devMode) wireDevStage();
