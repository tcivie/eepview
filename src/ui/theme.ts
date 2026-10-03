export type ThemePref = "system" | "light" | "dark";

export interface ThemeStore {
  get(): ThemePref;
  set(p: ThemePref): void;
}

const STORAGE_KEY = "eepview.theme";
const PREFS: readonly ThemePref[] = ["system", "light", "dark"];
const listeners = new Set<(p: ThemePref) => void>();

export function isThemePref(value: unknown): value is ThemePref {
  return typeof value === "string" && (PREFS as readonly string[]).includes(value);
}

function readSavedPref(): ThemePref {
  const saved = window.localStorage.getItem(STORAGE_KEY);
  return isThemePref(saved) ? saved : "system";
}

export const localThemeStore: ThemeStore = {
  get() {
    try {
      return readSavedPref();
    } catch {
      return "system";
    }
  },
  set(p) {
    try {
      window.localStorage.setItem(STORAGE_KEY, p);
    } catch {
      return;
    }
  },
};

let store: ThemeStore = localThemeStore;
let current: ThemePref = "system";

function reviewOverride(): ThemePref | null {
  const value = new URLSearchParams(window.location.search).get("theme");
  return isThemePref(value) ? value : null;
}

function systemIsDark(): boolean {
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export function applyTheme(pref: ThemePref): void {
  const root = document.documentElement;
  if (pref === "system") {
    root.removeAttribute("data-theme");
  } else {
    root.setAttribute("data-theme", pref);
  }
  const dark = pref === "dark" || (pref === "system" && systemIsDark());
  root.dataset.resolvedTheme = dark ? "dark" : "light";
  current = pref;
  for (const listener of listeners) listener(pref);
}

export function getThemePref(): ThemePref {
  return current;
}

export function setThemePref(pref: ThemePref): void {
  store.set(pref);
  applyTheme(pref);
}

export function setThemeStore(next: ThemeStore): void {
  store = next;
  applyTheme(reviewOverride() ?? store.get());
}

export function adoptPref(pref: ThemePref): void {
  if (reviewOverride() === null) applyTheme(pref);
}

export function onThemeChange(listener: (p: ThemePref) => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function followSystemWhenUnset(): void {
  if (current === "system") applyTheme("system");
}

function adoptPrefSavedByOtherPage(event: StorageEvent): void {
  if (event.key !== STORAGE_KEY || reviewOverride() !== null) return;
  applyTheme(isThemePref(event.newValue) ? event.newValue : "system");
}

export function initTheme(): void {
  applyTheme(reviewOverride() ?? store.get());
  window
    .matchMedia("(prefers-color-scheme: dark)")
    .addEventListener("change", followSystemWhenUnset);
  window.addEventListener("storage", adoptPrefSavedByOtherPage);
}
