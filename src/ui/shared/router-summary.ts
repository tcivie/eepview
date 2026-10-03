import { type RouterLike, type RouterView, routerView } from "../lib/router-view.ts";

export interface TextTarget {
  textContent: string | null;
}

export interface ChipTarget extends TextTarget {
  dataset: Record<string, string | undefined>;
}

export interface RouterSummaryTargets {
  chip: ChipTarget;
  text: TextTarget;
  proxy: TextTarget;
}

export const NO_PROXY = "—";

export function renderRouterSummary(targets: RouterSummaryTargets, status: RouterLike): RouterView {
  const view = routerView(status);
  targets.chip.dataset.tone = view.tone;
  targets.chip.textContent = view.label;
  targets.text.textContent = view.text;
  targets.proxy.textContent = status.proxy || NO_PROXY;
  return view;
}
