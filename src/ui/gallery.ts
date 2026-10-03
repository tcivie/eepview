import "./boot.ts";
import { all } from "./dom.ts";
import { getThemePref, isThemePref, onThemeChange, setThemePref, type ThemePref } from "./theme.ts";

function showPref(pref: ThemePref): void {
  for (const radio of all<HTMLInputElement>('input[name="theme"]')) {
    radio.checked = radio.value === pref;
  }
}

function onPicked(event: Event): void {
  const value = (event.target as HTMLInputElement).value;
  if (isThemePref(value)) setThemePref(value);
}

showPref(getThemePref());
onThemeChange(showPref);
for (const radio of all<HTMLInputElement>('input[name="theme"]')) {
  radio.addEventListener("change", onPicked);
}
