// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

export const CHART_WIDTH = 600;
export const CHART_HEIGHT = 140;
const CHART_TOP = 10;
const SCALE_STEPS = [1, 2, 5];
const MIN_RUN = 2;

/** A series of time slots; `null` is a slot with no sample. */
type Series = readonly (number | null)[];

interface Point {
  x: string;
  y: string;
}

const present = (v: number | null): v is number => v !== null;

export function scaleMax(values: Series): number {
  const peak = Math.max(1, ...values.filter(present));
  const magnitude = 10 ** Math.floor(Math.log10(peak));
  const step = SCALE_STEPS.find((s) => s * magnitude >= peak) ?? 10;
  return step * magnitude;
}

/** Each run of two or more non-null slots in a row, as chart points. */
function runs(values: Series, max: number): Point[][] {
  const step = CHART_WIDTH / Math.max(1, values.length - 1);
  const usable = CHART_HEIGHT - CHART_TOP;
  const found: Point[][] = [];
  let current: Point[] = [];
  values.forEach((v, i) => {
    if (v === null) {
      current = [];
      return;
    }
    if (current.length === 0) found.push(current);
    const y = CHART_HEIGHT - (Math.min(v, max) / max) * usable;
    current.push({ x: (i * step).toFixed(1), y: y.toFixed(1) });
  });
  return found.filter((run) => run.length >= MIN_RUN);
}

const line = (run: Point[]): string => `M${run.map((p) => `${p.x} ${p.y}`).join("L")}`;

/** One line per run; "" when no run has two points. */
export function linePath(values: Series, max: number): string {
  return runs(values, max).map(line).join("");
}

/** One closed area per run, down to the bottom of the chart; "" when no run has two points. */
export function areaPath(values: Series, max: number): string {
  return runs(values, max)
    .map((run) => {
      const first = run[0]?.x ?? "0";
      const last = run[run.length - 1]?.x ?? "0";
      return `${line(run)}L${last} ${CHART_HEIGHT}L${first} ${CHART_HEIGHT}Z`;
    })
    .join("");
}
