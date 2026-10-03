import "./boot.ts";
import { all, announce, byId } from "./dom.ts";
import {
  countText,
  formatElapsed,
  isStep,
  type Progress,
  percentOf,
  progressText,
  type Step,
  stepPosition,
  stepperHopState,
  totalProgress,
} from "./lib/setup-flow.ts";
import { delegateClick } from "./shared/events.ts";

const DOWNLOAD_TICK_MS = 200;
const DOWNLOAD_TICK_MB = 0.8;
const ROUTER_MB = 31;
const JAVA_MB = 44;
const ROUTER_START_MB = 19.4;
const BUILD_TICK_MS = 1000;
const HAND_OFF_MS = 1200;
const START_ELAPSED_S = 100;
const START_PEERS = 312;
const PEERS_PER_TICK = 7;
const TUNNELS = 2;
const MILESTONE_PERCENT = 25;

interface Transfer extends Progress {
  bar: HTMLProgressElement;
  size: HTMLElement;
  label: string;
}

interface BuildRun {
  queue: HTMLElement[];
  totalHops: number;
  elapsed: number;
  peers: number;
}

type Stop = () => void;
let stopRunning: Stop | null = null;
let handOffTimer = 0;

function updateStepper(step: Step): void {
  const position = stepPosition(step);
  byId("setup-progress").hidden = position === null;
  if (!position) return;
  byId("step-count").textContent = position.text;
  all<HTMLElement>(".hop", byId("stepper")).forEach((hop, index) => {
    const state = stepperHopState(index, position);
    if (index === position.index) hop.setAttribute("aria-current", "step");
    else hop.removeAttribute("aria-current");
    if (state) hop.dataset.state = state;
    else delete hop.dataset.state;
  });
}

function handOff(step: Step): void {
  window.clearTimeout(handOffTimer);
  handOffTimer = window.setTimeout(() => showStep(step, true), HAND_OFF_MS);
}

function showStep(step: Step, moveFocus: boolean): void {
  for (const section of all<HTMLElement>("[data-step]")) {
    section.hidden = section.dataset.step !== step;
  }
  updateStepper(step);
  window.clearTimeout(handOffTimer);
  stopRunning?.();
  stopRunning = RUNNERS[step]?.() ?? null;
  if (moveFocus) document.querySelector<HTMLElement>(`[data-step="${step}"] h1`)?.focus();
}

function transfer(id: string, totalMb: number, label: string): Transfer {
  return { bar: byId(`${id}-progress`), size: byId(`${id}-size`), totalMb, doneMb: 0, label };
}

function transfers(): Transfer[] {
  const router = { ...transfer("router", ROUTER_MB, "the I2P router"), doneMb: ROUTER_START_MB };
  const java = transfer("java", JAVA_MB, "Java");
  return usesSystemJava() ? [router] : [router, java];
}

function showProgress(items: Transfer[]): void {
  for (const item of items) {
    const percent = percentOf(item);
    item.bar.value = percent;
    item.bar.textContent = `${percent}%`;
    item.size.textContent = progressText(item);
  }
  byId("total-size").textContent = progressText(totalProgress(items));
}

function announceMilestone(item: Transfer, before: number): void {
  const percent = percentOf(item);
  if (Math.floor(percent / MILESTONE_PERCENT) === Math.floor(before / MILESTONE_PERCENT)) return;
  announce(byId("download-status"), `Downloading ${item.label}, ${percent} percent.`);
}

function advanceDownload(items: Transfer[]): boolean {
  const item = items.find((candidate) => candidate.doneMb < candidate.totalMb);
  if (!item) return true;
  const before = percentOf(item);
  item.doneMb = Math.min(item.totalMb, item.doneMb + DOWNLOAD_TICK_MB);
  showProgress(items);
  announceMilestone(item, before);
  return false;
}

function finishDownload(): void {
  byId("verify-state").textContent = "All signatures match";
  byId<HTMLButtonElement>("download-next").disabled = false;
  announce(byId("download-status"), "Download complete. All signatures match.");
  handOff("tunnels");
}

function runDownload(): Stop {
  const items = transfers();
  byId("java-transfer").hidden = usesSystemJava();
  byId<HTMLButtonElement>("download-next").disabled = true;
  showProgress(items);
  const timer = window.setInterval(() => {
    if (!advanceDownload(items)) return;
    window.clearInterval(timer);
    finishDownload();
  }, DOWNLOAD_TICK_MS);
  return () => window.clearInterval(timer);
}

function pendingHops(): HTMLElement[] {
  const outbound = all<HTMLElement>("#build-out .hop:not([data-state='built'])");
  const inbound = all<HTMLElement>("#build-in .hop:not([data-state='built'])").reverse();
  return [...outbound, ...inbound];
}

function tunnelsReady(): number {
  const done = (id: string) => all(`#${id} .hop:not([data-state='built'])`).length === 0;
  return Number(done("build-out")) + Number(done("build-in"));
}

function advanceHop(queue: HTMLElement[]): void {
  const hop = queue.shift();
  if (hop) hop.dataset.state = "built";
  const next = queue[0];
  if (next) next.dataset.state = "building";
}

function showBuildCounts(ready: number, hopsLeft: number, totalHops: number): void {
  byId("build-ready").textContent = countText(ready, TUNNELS);
  byId("build-hops").textContent = countText(totalHops - hopsLeft, totalHops);
  const message = ready === TUNNELS ? "Both tunnels are built." : "Building the next hop.";
  announce(byId("tunnel-status"), message);
}

function buildNextHop(queue: HTMLElement[], totalHops: number): void {
  advanceHop(queue);
  showBuildCounts(tunnelsReady(), queue.length, totalHops);
}

function tickBuild(run: BuildRun): boolean {
  run.elapsed += 1;
  run.peers += PEERS_PER_TICK;
  byId("build-elapsed").textContent = formatElapsed(run.elapsed);
  byId("build-peers").textContent = run.peers.toLocaleString("en");
  if (run.elapsed % 2 === 0) buildNextHop(run.queue, run.totalHops);
  return run.queue.length === 0;
}

function runTunnels(): Stop {
  const queue = pendingHops();
  const run: BuildRun = {
    queue,
    totalHops: queue.length,
    elapsed: START_ELAPSED_S,
    peers: START_PEERS,
  };
  byId<HTMLButtonElement>("tunnels-next").disabled = true;
  if (queue[0]) queue[0].dataset.state = "building";
  const timer = window.setInterval(() => {
    if (!tickBuild(run)) return;
    window.clearInterval(timer);
    byId<HTMLButtonElement>("tunnels-next").disabled = false;
    handOff("ready");
  }, BUILD_TICK_MS);
  return () => window.clearInterval(timer);
}

const RUNNERS: Partial<Record<Step, () => Stop>> = {
  download: runDownload,
  tunnels: runTunnels,
};

function usesSystemJava(): boolean {
  return document.querySelector<HTMLInputElement>('input[name="java"]:checked')?.value === "system";
}

function onJavaChoice(): void {
  const system = usesSystemJava();
  byId("java-row").classList.toggle("row-skipped", system);
  byId("download-total").textContent = `${system ? ROUTER_MB : ROUTER_MB + JAVA_MB} MB`;
}

function onGoto(target: HTMLElement): void {
  const step = target.dataset.goto;
  if (isStep(step)) showStep(step, true);
}

function onInstallerChosen(event: Event): void {
  const file = (event.target as HTMLInputElement).files?.[0];
  if (file) byId("have-file").textContent = `Install from ${file.name}`;
}

function keepThemeInReviewLinks(): void {
  const theme = new URLSearchParams(window.location.search).get("theme");
  if (!theme) return;
  for (const link of all<HTMLAnchorElement>(".review-nav a")) {
    const url = new URL(link.href);
    url.searchParams.set("theme", theme);
    link.href = url.href;
  }
}

function wire(): void {
  delegateClick(byId("main"), "[data-goto]", onGoto);
  for (const radio of all<HTMLInputElement>('input[name="java"]')) {
    radio.addEventListener("change", onJavaChoice);
  }
  byId("have-file").addEventListener("click", () => byId("installer-file").click());
  byId("installer-file").addEventListener("change", onInstallerChosen);
  keepThemeInReviewLinks();
}

wire();
const requested = new URLSearchParams(window.location.search).get("step");
showStep(isStep(requested) ? requested : "install", false);
