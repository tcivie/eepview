import type { ChromeInsets } from "../contract.ts";
import { all, byId } from "../dom.ts";
import { call, on } from "../ipc.ts";
import { chromeHeight, insetPx } from "../lib/chrome-height.ts";

const POPUPS = ".suggestions, .menu, .tooltip, .router-panel";
const quiet = (): undefined => undefined;
let lastHeight = 0;
let pending = 0;

function measure(): number {
  const open = all<HTMLElement>(POPUPS).filter((el) => !el.hidden);
  return chromeHeight({
    findOpen: !byId("findbar").hidden,
    popupBottoms: open.map((el) => el.getBoundingClientRect().bottom),
  });
}

function syncHeight(): void {
  pending = 0;
  const height = measure();
  if (height === lastHeight) return;
  lastHeight = height;
  call("chrome_set_height", { px: height }).catch(quiet);
}

function scheduleSync(): void {
  if (pending === 0) pending = window.requestAnimationFrame(syncHeight);
}

function applyPlatform(platform: string): void {
  document.documentElement.dataset.platform = platform;
}

function applyInsets(insets: ChromeInsets): void {
  document.documentElement.style.setProperty("--chrome-inset-left", insetPx(insets.left));
}

function applyFullscreen(fullscreen: boolean): void {
  document.documentElement.toggleAttribute("data-fullscreen", fullscreen);
}

export function wireLayout(): void {
  const observer = new MutationObserver(scheduleSync);
  observer.observe(byId("chrome-bar"), {
    subtree: true,
    childList: true,
    attributes: true,
    attributeFilter: ["hidden"],
  });
  call("platform", {}).then(applyPlatform).catch(quiet);
  on("fullscreen-changed", applyFullscreen).catch(quiet);
  call("window_fullscreen", {}).then(applyFullscreen).catch(quiet);
  on("chrome-insets-changed", applyInsets).catch(quiet);
  call("chrome_insets", {}).then(applyInsets).catch(quiet);
  scheduleSync();
}
