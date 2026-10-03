import "./boot.ts";
import { all } from "./dom.ts";
import { checkMatching } from "./shared/form.ts";
import { getThemePref, isThemePref, onThemeChange, setThemePref, type ThemePref } from "./theme.ts";

function showPref(pref: ThemePref): void {
  checkMatching(all<HTMLInputElement>('input[name="theme"]'), pref);
}

function onPicked(event: Event): void {
  const value = (event.target as HTMLInputElement).value;
  if (isThemePref(value)) setThemePref(value);
}

function addDevFlag(): void {
  for (const link of all<HTMLAnchorElement>('a[href*=".html"]')) {
    const url = new URL(link.href);
    url.searchParams.set("dev", "1");
    link.href = url.href;
  }
}

addDevFlag();
showPref(getThemePref());
onThemeChange(showPref);
for (const radio of all<HTMLInputElement>('input[name="theme"]')) {
  radio.addEventListener("change", onPicked);
}
