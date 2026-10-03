import "./boot.ts";
import { announce, byId } from "./dom.ts";

type Phase = "waiting" | "trying" | "gave-up";

const TICK_MS = 1000;
const WAIT_S = 12;
const TRY_S = 3;
const MAX_ATTEMPTS = 5;

interface RetryState {
  phase: Phase;
  secondsLeft: number;
  attempt: number;
}

const state: RetryState = { phase: "waiting", secondsLeft: WAIT_S, attempt: 2 };

const PHASE_CHIP: Record<Phase, { tone: string; label: string }> = {
  waiting: { tone: "stopped", label: "Stopped" },
  trying: { tone: "building", label: "Starting" },
  "gave-up": { tone: "stopped", label: "Stopped" },
};

function phaseText(s: RetryState): string {
  if (s.phase === "trying") return "Starting the router. This takes a few seconds.";
  if (s.phase === "gave-up")
    return "Automatic restarts stopped after 5 tries. Restart it yourself.";
  return `Restarting the router in ${s.secondsLeft} s.`;
}

let announcedPhase: Phase | null = null;

function announcePhaseChange(): void {
  if (announcedPhase === state.phase) return;
  announcedPhase = state.phase;
  announce(byId("retry-status"), phaseText(state));
}

function render(): void {
  announcePhaseChange();
  const chip = byId("retry-chip");
  chip.dataset.tone = PHASE_CHIP[state.phase].tone;
  chip.textContent = PHASE_CHIP[state.phase].label;
  byId("retry-text").textContent = phaseText(state);
  byId("retry-attempt").textContent = String(state.attempt);
  byId("retry").dataset.phase = state.phase;
  byId<HTMLButtonElement>("retry-now").disabled = state.phase === "trying";
}

function startTry(): void {
  state.phase = "trying";
  state.secondsLeft = TRY_S;
}

function tryFailed(): void {
  state.attempt += 1;
  state.phase = state.attempt > MAX_ATTEMPTS ? "gave-up" : "waiting";
  state.attempt = Math.min(state.attempt, MAX_ATTEMPTS);
  state.secondsLeft = WAIT_S;
}

function tick(): void {
  if (state.phase === "gave-up") return;
  state.secondsLeft -= 1;
  if (state.secondsLeft > 0) {
    render();
    return;
  }
  if (state.phase === "waiting") startTry();
  else tryFailed();
  render();
}

byId("retry-now").addEventListener("click", () => {
  startTry();
  render();
});
render();
window.setInterval(tick, TICK_MS);
