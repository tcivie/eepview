// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import "./boot.ts";
import { renderConsoleLink, shownInActiveTab, wireConsoleClicks } from "./console-nav.ts";
import type { ConsoleInfo, RouterStatus, Settings } from "./contract.ts";
import { all, announce, byId } from "./dom.ts";
import { call, devMode, on } from "./ipc.ts";
import { shouldRedetect } from "./lib/console-links.ts";
import { versionText } from "./lib/router-view.ts";
import {
  type HomepageMode,
  homepageAddress,
  homepageMode,
  homepageValue,
  nearestZoom,
} from "./lib/settings-form.ts";
import { runAndAnnounce } from "./shared/events.ts";
import { checkMatching, setFieldError } from "./shared/form.ts";
import { proxyText } from "./shared/router-summary.ts";
import { getThemePref, isThemePref, onThemeChange, setThemePref, type ThemePref } from "./theme.ts";

const HOMEPAGE_HINT = "Saved when you leave the field.";
const status = (): HTMLElement => byId("settings-status");
const checkbox = (id: string): HTMLInputElement => byId<HTMLInputElement>(id);

const SWITCHES: Record<string, (on: boolean) => Partial<Settings>> = {
  "js-default": (on) => ({ jsDefault: on }),
  "keep-cookies": (on) => ({ keepCookies: on }),
  "history-on": (on) => ({ history: { enabled: on } }),
};

function themeRadios(): HTMLInputElement[] {
  return all<HTMLInputElement>('input[name="theme"]');
}

function showThemePref(pref: ThemePref): void {
  checkMatching(themeRadios(), pref);
}

function onThemePicked(event: Event): void {
  const value = (event.target as HTMLInputElement).value;
  if (!isThemePref(value)) return;
  setThemePref(value);
  // The shell paints the window and new tabs from the saved theme, so tell it.
  call("settings_set", { patch: { theme: value } }).catch(() => undefined);
  announce(status(), `Theme set to ${value}.`);
}

function showHomepage(homepage: string): void {
  const mode = homepageMode(homepage);
  checkMatching(all<HTMLInputElement>('input[name="homepage"]'), mode);
  const field = byId<HTMLInputElement>("homepage-url");
  field.disabled = mode === "home";
  if (document.activeElement !== field) field.value = homepageAddress(homepage);
}

function showSettings(settings: Settings): void {
  checkbox("js-default").checked = settings.jsDefault;
  checkbox("keep-cookies").checked = settings.keepCookies;
  checkbox("history-on").checked = settings.history.enabled;
  const zoom = byId<HTMLSelectElement>("zoom-default");
  const offered = [...zoom.options].map((o) => Number(o.value));
  zoom.value = String(nearestZoom(settings.zoomDefault, offered));
  showHomepage(settings.homepage);
}

function showSavedSettings(): void {
  call("settings_get", {})
    .then(showSettings)
    .catch(() => undefined);
}

function save(patch: Partial<Settings>, message: string): void {
  const task = () => call("settings_set", { patch }).then(showSettings);
  runAndAnnounce(task, status(), message).then((saved) => {
    if (!saved) showSavedSettings();
  });
}

function onSwitch(event: Event): void {
  const input = event.target as HTMLInputElement;
  const patch = SWITCHES[input.id]?.(input.checked);
  const label = document.querySelector(`label[for="${input.id}"]`)?.textContent ?? "Setting";
  if (patch) save(patch, `${label}: ${input.checked ? "on" : "off"}.`);
}

function setHomepageError(message: string | null): void {
  setFieldError(byId("homepage-url"), byId("homepage-hint"), message, HOMEPAGE_HINT);
}

function saveHomepage(mode: HomepageMode): void {
  const value = homepageValue(mode, byId<HTMLInputElement>("homepage-url").value);
  setHomepageError(value ? null : "This is not an I2P address. It must end in .i2p.");
  if (value) save({ homepage: value }, "Homepage saved.");
}

function onHomepageMode(event: Event): void {
  const mode = (event.target as HTMLInputElement).value as HomepageMode;
  const field = byId<HTMLInputElement>("homepage-url");
  field.disabled = mode === "home";
  if (mode === "home") saveHomepage(mode);
  else field.focus();
}

function wireTheme(): void {
  showThemePref(getThemePref());
  onThemeChange(showThemePref);
  for (const radio of themeRadios()) radio.addEventListener("change", onThemePicked);
}

function wireSettings(): void {
  for (const id of Object.keys(SWITCHES)) byId(id).addEventListener("change", onSwitch);
  byId("zoom-default").addEventListener("change", (event) => {
    const value = Number((event.target as HTMLSelectElement).value);
    save({ zoomDefault: value }, `Default zoom set to ${Math.round(value * 100)}%.`);
  });
  for (const radio of all<HTMLInputElement>('input[name="homepage"]')) {
    radio.addEventListener("change", onHomepageMode);
  }
  byId("homepage-url").addEventListener("change", () => saveHomepage("custom"));
  call("settings_get", {})
    .then(showSettings)
    .catch(() => undefined);
  on("settings-changed", showSettings).catch(() => undefined);
}

function showRouter(router: RouterStatus): void {
  byId("connect-proxy").textContent = proxyText(router);
  byId("about-router").textContent = versionText(router);
}

function wireRouter(): void {
  call("router_status", {})
    .then(showRouter)
    .catch(() => undefined);
  on("router-status", showRouter).catch(() => undefined);
}

function renderConsole(info: ConsoleInfo): void {
  renderConsoleLink({ list: byId("console-links"), note: byId("console-note") }, info);
}

let lastStatus: RouterStatus | null = null;

function redetectWhenReady(status: RouterStatus): void {
  const ready = shouldRedetect(lastStatus, status);
  lastStatus = status;
  if (!ready) return;
  shownInActiveTab("settings")
    .then((shown) => (shown ? call("console_detect", {}).then(renderConsole) : undefined))
    .catch(() => undefined);
}

function wireConsole(): void {
  call("router_status", {})
    .then((status) => {
      lastStatus = status;
    })
    .catch(() => undefined);
  on("router-status", redetectWhenReady).catch(() => undefined);
  wireConsoleClicks(byId("console-links"));
  shownInActiveTab("settings")
    .catch(() => false)
    .then((shown) => call(shown ? "console_detect" : "console_status", {}))
    .then(renderConsole)
    .catch(() => undefined);
  on("console-changed", renderConsole).catch(() => undefined);
}

function wireRouterPreview(): void {
  byId("router-preview").hidden = devMode;
  for (const button of all<HTMLButtonElement>("[data-grant], #restore-btn")) {
    button.disabled = !devMode;
  }
}

function wireDeleteLogs(): void {
  byId("delete-logs").addEventListener("click", () => {
    const task = () => call("diag_logs_delete", {});
    runAndAnnounce(task, status(), "Diagnostics logs deleted.").catch(() => undefined);
  });
}

wireTheme();
wireSettings();
wireRouter();
wireDeleteLogs();
wireRouterPreview();
wireConsole();
