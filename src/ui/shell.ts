import type { Settings } from "./contract.ts";
import { call, devMode, inShell, on } from "./ipc.ts";
import { internalUrlForFile, isI2pAddress, isInternal } from "./lib/address.ts";
import { adoptPref, localThemeStore, setThemeStore, type ThemeStore } from "./theme.ts";

const REVIEW_PARAMS = ["dev", "theme", "router"];

const settingsThemeStore: ThemeStore = {
  get: () => localThemeStore.get(),
  set: (pref) => {
    localThemeStore.set(pref);
    call("settings_set", { patch: { theme: pref } }).catch(() => undefined);
  },
};

function adoptSettings(settings: Settings): void {
  localThemeStore.set(settings.theme);
  adoptPref(settings.theme);
}

function syncTheme(): void {
  setThemeStore(settingsThemeStore);
  call("settings_get", {})
    .then(adoptSettings)
    .catch(() => undefined);
  on("settings-changed", adoptSettings).catch(() => undefined);
}

function shellTarget(href: string): string | null {
  if (isInternal(href)) return href;
  if (isI2pAddress(href) && /^https?:/i.test(href)) return href;
  return internalUrlForFile(href);
}

function onLinkClick(event: MouseEvent): void {
  const link = (event.target as Element | null)?.closest<HTMLAnchorElement>("a[href]");
  const href = link?.getAttribute("href");
  if (!href || href.startsWith("#") || event.defaultPrevented) return;
  const target = shellTarget(href);
  if (!target) return;
  event.preventDefault();
  call("navigate", { input: target }).catch(() => undefined);
}

function carryReviewParams(): void {
  const current = new URLSearchParams(window.location.search);
  const carried = REVIEW_PARAMS.filter((key) => current.has(key));
  if (carried.length === 0) return;
  for (const link of document.querySelectorAll<HTMLAnchorElement>('a[href$=".html"]')) {
    const url = new URL(link.href);
    for (const key of carried) url.searchParams.set(key, current.get(key) ?? "");
    link.href = url.href;
  }
}

export function connectShell(): void {
  syncTheme();
  if (inShell) document.addEventListener("click", onLinkClick);
  if (devMode) document.addEventListener("DOMContentLoaded", carryReviewParams);
  if (devMode && document.readyState !== "loading") carryReviewParams();
}
