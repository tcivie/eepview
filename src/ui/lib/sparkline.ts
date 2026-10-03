export const CHART_WIDTH = 600;
export const CHART_HEIGHT = 140;
const CHART_TOP = 10;
const SCALE_STEPS = [1, 2, 5];

export function scaleMax(values: number[]): number {
  const peak = Math.max(1, ...values);
  const magnitude = 10 ** Math.floor(Math.log10(peak));
  const step = SCALE_STEPS.find((s) => s * magnitude >= peak) ?? 10;
  return step * magnitude;
}

function points(values: number[], max: number): string[] {
  const step = CHART_WIDTH / Math.max(1, values.length - 1);
  const usable = CHART_HEIGHT - CHART_TOP;
  return values.map((v, i) => {
    const x = (i * step).toFixed(1);
    const y = (CHART_HEIGHT - (Math.min(v, max) / max) * usable).toFixed(1);
    return `${x} ${y}`;
  });
}

export function linePath(values: number[], max: number): string {
  return `M${points(values, max).join("L")}`;
}

export function areaPath(values: number[], max: number): string {
  return `${linePath(values, max)}L${CHART_WIDTH} ${CHART_HEIGHT}L0 ${CHART_HEIGHT}Z`;
}
