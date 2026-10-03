import "./boot.ts";
import { byId } from "./dom.ts";

const CHART_WIDTH = 600;
const CHART_HEIGHT = 140;
const CHART_TOP = 10;
const SAMPLES = 60;
const REFRESH_MS = 2000;
const SCALE_STEP_KBPS = 20;

interface Series {
  inbound: number[];
  outbound: number[];
}

function wave(i: number, base: number, swing: number, phase: number): number {
  const slow = Math.sin((i + phase) / 7) * swing;
  const fast = Math.sin((i + phase) * 1.7) * swing * 0.35;
  return Math.max(2, base + slow + fast);
}

function sampleSeries(): Series {
  const indexes = Array.from({ length: SAMPLES }, (_, i) => i);
  return {
    inbound: indexes.map((i) => wave(i, 44, 14, 0)),
    outbound: indexes.map((i) => wave(i, 29, 9, 11)),
  };
}

function scaleMax(series: Series): number {
  const peak = Math.max(...series.inbound, ...series.outbound);
  return Math.ceil(peak / SCALE_STEP_KBPS) * SCALE_STEP_KBPS;
}

function points(values: number[], max: number): string[] {
  const step = CHART_WIDTH / (values.length - 1);
  const usable = CHART_HEIGHT - CHART_TOP;
  return values.map((v, i) => {
    const x = (i * step).toFixed(1);
    const y = (CHART_HEIGHT - (v / max) * usable).toFixed(1);
    return `${x} ${y}`;
  });
}

export function linePath(values: number[], max: number): string {
  return `M${points(values, max).join("L")}`;
}

export function areaPath(values: number[], max: number): string {
  return `${linePath(values, max)}L${CHART_WIDTH} ${CHART_HEIGHT}L0 ${CHART_HEIGHT}Z`;
}

function last(values: number[]): number {
  return values[values.length - 1] ?? 0;
}

function renderChart(series: Series): void {
  const max = scaleMax(series);
  byId("spark-in").setAttribute("d", linePath(series.inbound, max));
  byId("spark-in-area").setAttribute("d", areaPath(series.inbound, max));
  byId("spark-out").setAttribute("d", linePath(series.outbound, max));
  byId("spark-max").textContent = `${max} KB/s`;
  const inNow = last(series.inbound).toFixed(1);
  const outNow = last(series.outbound).toFixed(1);
  byId("bw-in-now").textContent = inNow;
  byId("bw-out-now").textContent = outNow;
  byId("spark-summary").textContent =
    `Bandwidth over the last 10 minutes. Now ${inNow} KB/s in and ${outNow} KB/s out.`;
}

function advance(series: Series, tick: number): Series {
  return {
    inbound: [...series.inbound.slice(1), wave(SAMPLES + tick, 44, 14, 0)],
    outbound: [...series.outbound.slice(1), wave(SAMPLES + tick, 29, 9, 11)],
  };
}

let series = sampleSeries();
let tick = 0;
renderChart(series);
window.setInterval(() => {
  tick += 1;
  series = advance(series, tick);
  renderChart(series);
}, REFRESH_MS);
