import { displayUrl, eepsiteUrl, isI2pAddress, isInternal } from "./address.ts";

export const HOME_PAGE = "eepview://home";

export type HomepageMode = "home" | "custom";

export function homepageMode(homepage: string): HomepageMode {
  return homepage.trim() === "" || homepage === HOME_PAGE ? "home" : "custom";
}

export function homepageAddress(homepage: string): string {
  return homepageMode(homepage) === "home" ? "" : displayUrl(homepage);
}

export function homepageValue(mode: HomepageMode, address: string): string | null {
  if (mode === "home") return HOME_PAGE;
  const trimmed = address.trim();
  if (isInternal(trimmed)) return trimmed;
  return isI2pAddress(trimmed) ? eepsiteUrl(trimmed) : null;
}

export function nearestZoom(zoom: number, options: number[]): number {
  let best = options[0] ?? 1;
  for (const option of options) {
    if (Math.abs(option - zoom) < Math.abs(best - zoom)) best = option;
  }
  return best;
}
