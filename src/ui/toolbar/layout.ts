import { all, byId } from "../dom.ts";
import { call } from "../ipc.ts";
import { chromeHeight, tabStripInset } from "../lib/chrome-height.ts";

const POPUPS = ".suggestions, .menu, .tooltip";
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
  document.body.dataset.platform = platform;
  byId("tabstrip").style.setProperty("--tabstrip-inset", `${tabStripInset(platform)}px`);
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
  scheduleSync();
}
