export type Step =
  | "install"
  | "found"
  | "download"
  | "download-failed"
  | "tunnels"
  | "tunnels-failed"
  | "ready"
  | "later";

export type HopState = "built" | "building" | "refused" | null;

export interface StepPosition {
  index: number;
  failed: boolean;
  complete: boolean;
  text: string;
}

export interface Progress {
  doneMb: number;
  totalMb: number;
}

export const STEP_TOTAL = 4;
const PERCENT = 100;
const SECONDS_PER_MINUTE = 60;

const STEP_INDEX: Record<Step, number | null> = {
  install: 0,
  found: 0,
  download: 1,
  "download-failed": 1,
  tunnels: 2,
  "tunnels-failed": 2,
  ready: 3,
  later: null,
};

export function isStep(value: unknown): value is Step {
  return typeof value === "string" && Object.keys(STEP_INDEX).includes(value);
}

export function stepPosition(step: Step): StepPosition | null {
  const index = STEP_INDEX[step];
  if (index === null) return null;
  return {
    index,
    failed: step.endsWith("-failed"),
    complete: step === "ready",
    text: `Step ${index + 1} of ${STEP_TOTAL}`,
  };
}

export function stepperHopState(hop: number, position: StepPosition): HopState {
  if (hop < position.index || position.complete) return "built";
  if (hop > position.index) return null;
  return position.failed ? "refused" : "building";
}

export function percentOf(progress: Progress): number {
  if (progress.totalMb <= 0) return 0;
  return Math.min(PERCENT, Math.floor((progress.doneMb / progress.totalMb) * PERCENT));
}

export function progressText(progress: Progress): string {
  return `${progress.doneMb.toFixed(1)} of ${progress.totalMb} MB · ${percentOf(progress)}%`;
}

export function totalProgress(items: readonly Progress[]): Progress {
  return items.reduce(
    (sum, item) => ({ doneMb: sum.doneMb + item.doneMb, totalMb: sum.totalMb + item.totalMb }),
    { doneMb: 0, totalMb: 0 },
  );
}

export function formatElapsed(seconds: number): string {
  const minutes = Math.floor(seconds / SECONDS_PER_MINUTE);
  const rest = seconds % SECONDS_PER_MINUTE;
  return minutes > 0 ? `${minutes} min ${rest} s` : `${rest} s`;
}

export function countText(done: number, total: number): string {
  return `${done} of ${total}`;
}
