import "./boot.ts";
import { all, announce, byId } from "./dom.ts";
import { delegateClick } from "./shared/events.ts";

type Step = "install" | "download" | "tunnels" | "ready" | "found" | "later";

const STEPS: readonly Step[] = ["install", "download", "tunnels", "ready", "found", "later"];
const SEQUENCE: readonly Step[] = ["install", "download", "tunnels", "ready"];
const DOWNLOAD_TICK_MS = 200;
const DOWNLOAD_STEP_PERCENT = 2;
const BUILD_TICK_MS = 1000;
const HAND_OFF_MS = 1200;
const START_ELAPSED_S = 100;
const START_PEERS = 312;

type Stop = () => void;
let stopRunning: Stop | null = null;

function isStep(value: unknown): value is Step {
  return typeof value === "string" && (STEPS as readonly string[]).includes(value);
}

function hopState(index: number, current: number, step: Step): string | null {
  if (index < current || step === "ready") return "built";
  return index === current ? "building" : null;
}

function updateStepper(step: Step): void {
  const current = SEQUENCE.indexOf(step);
  const stepper = byId("stepper");
  stepper.hidden = current < 0;
  all<HTMLElement>(".hop", stepper).forEach((hop, index) => {
    const state = hopState(index, current, step);
    hop.toggleAttribute("aria-current", index === current);
    if (index === current) hop.setAttribute("aria-current", "step");
    if (state) hop.dataset.state = state;
    else delete hop.dataset.state;
  });
}

function showStep(step: Step, moveFocus: boolean): void {
  for (const section of all<HTMLElement>("[data-step]")) {
    section.hidden = section.dataset.step !== step;
  }
  updateStepper(step);
  stopRunning?.();
  stopRunning = RUNNERS[step]?.() ?? null;
  if (moveFocus) document.querySelector<HTMLElement>(`[data-step="${step}"] h1`)?.focus();
}

interface Transfer {
  bar: HTMLProgressElement;
  size: HTMLElement;
  totalMb: number;
  label: string;
}

interface DownloadRun {
  queue: Transfer[];
  percent: number;
}

function transfers(): Transfer[] {
  const router: Transfer = {
    bar: byId("router-progress"),
    size: byId("router-size"),
    totalMb: 31,
    label: "the I2P router",
  };
  const java: Transfer = {
    bar: byId("java-progress"),
    size: byId("java-size"),
    totalMb: 44,
    label: "Java",
  };
  return usesSystemJava() ? [router] : [router, java];
}

function showProgress(transfer: Transfer, percent: number): void {
  const doneMb = ((transfer.totalMb * percent) / 100).toFixed(1);
  transfer.bar.value = percent;
  transfer.bar.textContent = `${percent}%`;
  transfer.size.textContent = `${doneMb} of ${transfer.totalMb} MB`;
}

function finishDownload(): void {
  byId("verify-state").textContent = "Signatures match";
  announce(byId("download-status"), "Download complete. Signatures match. Starting the router.");
  window.setTimeout(() => showStep("tunnels", true), HAND_OFF_MS);
}

function announceMilestone(transfer: Transfer, percent: number): void {
  if (percent % 25 !== 0) return;
  announce(byId("download-status"), `Downloading ${transfer.label}, ${percent} percent.`);
}

function advanceDownload(run: DownloadRun): boolean {
  const transfer = run.queue[0];
  if (!transfer) return true;
  run.percent = Math.min(100, run.percent + DOWNLOAD_STEP_PERCENT);
  showProgress(transfer, run.percent);
  announceMilestone(transfer, run.percent);
  if (run.percent < 100) return false;
  run.queue.shift();
  run.percent = 0;
  return run.queue.length === 0;
}

function runDownload(): Stop {
  const queue = transfers();
  const run: DownloadRun = { queue, percent: queue[0]?.bar.value ?? 0 };
  const timer = window.setInterval(() => {
    if (!advanceDownload(run)) return;
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

function formatElapsed(seconds: number): string {
  return `${Math.floor(seconds / 60)} min ${seconds % 60} s`;
}

function tunnelsReady(): number {
  const done = (id: string) => all(`#${id} .hop:not([data-state='built'])`).length === 0;
  return Number(done("build-out")) + Number(done("build-in"));
}

function buildNextHop(queue: HTMLElement[]): void {
  const hop = queue.shift();
  if (hop) hop.dataset.state = "built";
  const next = queue[0];
  if (next) next.dataset.state = "building";
  const ready = tunnelsReady();
  byId("build-ready").textContent = `${ready} of 2`;
  const message = ready === 2 ? "Both tunnels are built." : "Building the next hop.";
  announce(byId("tunnel-status"), message);
}

function runTunnels(): Stop {
  const queue = pendingHops();
  const first = queue[0];
  if (first) first.dataset.state = "building";
  let elapsed = START_ELAPSED_S;
  let peers = START_PEERS;
  const timer = window.setInterval(() => {
    elapsed += 1;
    peers += 7;
    byId("build-elapsed").textContent = formatElapsed(elapsed);
    byId("build-peers").textContent = peers.toLocaleString("en");
    if (elapsed % 2 === 0) buildNextHop(queue);
    if (queue.length > 0) return;
    window.clearInterval(timer);
    window.setTimeout(() => showStep("ready", true), HAND_OFF_MS);
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
  byId("download-total").textContent = system ? "About 31 MB" : "About 75 MB";
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
