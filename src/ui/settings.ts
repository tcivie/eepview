// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import "./boot.ts";
import type { Settings } from "./contract.ts";
import { all, announce, byId } from "./dom.ts";
import { call, devMode, on } from "./ipc.ts";
import {
  type HomepageMode,
  homepageAddress,
  homepageMode,
  homepageValue,
  nearestZoom,
} from "./lib/settings-form.ts";
import { runAndAnnounce } from "./shared/events.ts";
import { checkMatching, setFieldError } from "./shared/form.ts";
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

function wireRouterPreview(): void {
  const range = byId<HTMLInputElement>("share");
  range.addEventListener("input", () => {
    byId<HTMLOutputElement>("share-out").value = `${range.value}%`;
  });
  byId("router-preview").hidden = devMode;
  for (const button of all<HTMLButtonElement>("[data-grant], #restore-btn")) {
    button.disabled = !devMode;
  }
}

wireTheme();
wireSettings();
wireRouterPreview();
