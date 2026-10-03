import "./boot.ts";
import { all, announce, byId } from "./dom.ts";

type RouterState = "building" | "ready" | "reconnecting" | "stopped";

interface StateCopy {
  label: string;
  title: string;
  text: string;
}

const STATE_COPY: Record<RouterState, StateCopy> = {
  building: {
    label: "Building",
    title: "Building tunnels",
    text: "The first start takes 2 to 10 minutes. Sites open when tunnels are up.",
  },
  ready: {
    label: "Ready",
    title: "Router ready",
    text: "6 tunnels up. Sites open through 3 hops.",
  },
  reconnecting: {
    label: "Reconnecting",
    title: "Reconnecting",
    text: "The network changed. The router is rebuilding tunnels. Pages may load slowly.",
  },
  stopped: {
    label: "Stopped",
    title: "Router stopped",
    text: "Eepsites cannot load. eepview restarts the router in a few seconds.",
  },
};

const I2P_HOST = /^(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+i2p$/;
const status = (): HTMLElement => byId("toolbar-status");

function isRouterState(value: string): value is RouterState {
  return value in STATE_COPY;
}

function setRouterState(state: RouterState): void {
  const copy = STATE_COPY[state];
  const button = byId("router-status");
  button.dataset.state = state;
  button.setAttribute("aria-label", `Router: ${copy.label}`);
  byId("router-status-label").textContent = copy.label;
  byId("status-pop-title").textContent = copy.title;
  byId("status-pop-text").textContent = copy.text;
  announce(status(), `Router: ${copy.title}.`);
}

function togglePressed(button: HTMLElement): boolean {
  const pressed = button.getAttribute("aria-pressed") !== "true";
  button.setAttribute("aria-pressed", String(pressed));
  return pressed;
}

function onJsToggle(): void {
  const on = togglePressed(byId("js-toggle"));
  announce(status(), on ? "JavaScript on for notbob.i2p." : "JavaScript off for notbob.i2p.");
}

function onStar(): void {
  const star = byId("star");
  const saved = togglePressed(star);
  star.setAttribute("aria-label", saved ? "Bookmarked" : "Bookmark this site");
  announce(status(), saved ? "Bookmarked." : "Bookmark removed.");
}

function setOpen(trigger: HTMLElement, open: boolean): void {
  const panel = byId(trigger.getAttribute("aria-controls") ?? "");
  trigger.setAttribute("aria-expanded", String(open));
  panel.hidden = !open;
}

function closeAll(): void {
  for (const trigger of all<HTMLElement>("[aria-controls][aria-expanded]")) setOpen(trigger, false);
}

function onTrigger(event: MouseEvent): void {
  const trigger = event.currentTarget as HTMLElement;
  const willOpen = trigger.getAttribute("aria-expanded") !== "true";
  closeAll();
  setOpen(trigger, willOpen);
  event.stopPropagation();
  if (willOpen)
    byId(trigger.getAttribute("aria-controls") ?? "")
      .querySelector("a")
      ?.focus();
}

function hostOf(address: string): string {
  const trimmed = address
    .trim()
    .toLowerCase()
    .replace(/^[a-z]+:/, "");
  return trimmed.replace(/^\/+/, "").split(/[/?#]/)[0] ?? "";
}

function onAddress(event: SubmitEvent): void {
  event.preventDefault();
  const address = byId<HTMLInputElement>("address-input").value.trim();
  if (!I2P_HOST.test(hostOf(address))) {
    window.location.href = `./blocked.html?url=${encodeURIComponent(address)}`;
    return;
  }
  byId("viewport-title").textContent = address;
  announce(status(), `Opening ${hostOf(address)}.`);
}

function onStatePicked(event: Event): void {
  const value = (event.target as HTMLInputElement).value;
  if (isRouterState(value)) setRouterState(value);
}

function wire(): void {
  byId("js-toggle").addEventListener("click", onJsToggle);
  byId("star").addEventListener("click", onStar);
  byId("reload").addEventListener("click", () => announce(status(), "Reloading."));
  byId("router-status").addEventListener("click", onTrigger);
  byId("menu-btn").addEventListener("click", onTrigger);
  byId<HTMLFormElement>("address-form").addEventListener("submit", onAddress);
  document.addEventListener("click", closeAll);
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") closeAll();
  });
  for (const radio of all<HTMLInputElement>('input[name="router-state"]')) {
    radio.addEventListener("change", onStatePicked);
  }
}

const REVIEW_TRIGGERS: Record<string, string> = { status: "router-status", menu: "menu-btn" };

function showRequestedState(params: URLSearchParams): void {
  const requested = params.get("router") ?? "";
  if (!isRouterState(requested)) return;
  setRouterState(requested);
  for (const radio of all<HTMLInputElement>('input[name="router-state"]')) {
    radio.checked = radio.value === requested;
  }
}

function openRequestedPanel(params: URLSearchParams): void {
  const triggerId = REVIEW_TRIGGERS[params.get("open") ?? ""];
  if (triggerId) setOpen(byId(triggerId), true);
}

wire();
const reviewParams = new URLSearchParams(window.location.search);
showRequestedState(reviewParams);
openRequestedPanel(reviewParams);
