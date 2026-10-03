import "./boot.ts";
import { all, announce, byId } from "./dom.ts";
import { getThemePref, isThemePref, onThemeChange, setThemePref, type ThemePref } from "./theme.ts";

const status = (): HTMLElement => byId("settings-status");

function themeRadios(): HTMLInputElement[] {
  return all<HTMLInputElement>('input[name="theme"]');
}

function showThemePref(pref: ThemePref): void {
  for (const radio of themeRadios()) radio.checked = radio.value === pref;
}

function onThemePicked(event: Event): void {
  const value = (event.target as HTMLInputElement).value;
  if (!isThemePref(value)) return;
  setThemePref(value);
  announce(status(), `Theme set to ${value}.`);
}

function wireTheme(): void {
  showThemePref(getThemePref());
  onThemeChange(showThemePref);
  for (const radio of themeRadios()) radio.addEventListener("change", onThemePicked);
}

function enableAddressForCustomHomepage(event: Event): void {
  const choice = (event.target as HTMLInputElement).value;
  byId<HTMLInputElement>("homepage-url").disabled = choice !== "custom";
}

function wireHomepage(): void {
  for (const radio of all<HTMLInputElement>('input[name="homepage"]')) {
    radio.addEventListener("change", enableAddressForCustomHomepage);
  }
}

function wireShare(): void {
  const range = byId<HTMLInputElement>("share");
  const output = byId<HTMLOutputElement>("share-out");
  range.addEventListener("input", () => {
    output.value = `${range.value}%`;
  });
}

function revoke(button: HTMLButtonElement): void {
  const grant = button.closest(".grant");
  const state = grant?.querySelector(".grant-state");
  const label = grant?.querySelector(".setting-label")?.textContent ?? "Permission";
  if (state) state.textContent = "Revoked. eepview will ask before doing this again.";
  button.disabled = true;
  button.textContent = "Revoked";
  announce(status(), `${label}: revoked.`);
}

function wireGrants(): void {
  for (const button of all<HTMLButtonElement>("button[data-grant]")) {
    button.addEventListener("click", () => revoke(button));
  }
}

function wireRestore(): void {
  const button = byId<HTMLButtonElement>("restore-btn");
  button.addEventListener("click", () => {
    button.disabled = true;
    button.textContent = "Restoring";
    announce(status(), "Restoring I2P 2.9.0. The router restarts when it is done.");
  });
}

wireTheme();
wireHomepage();
wireShare();
wireGrants();
wireRestore();
