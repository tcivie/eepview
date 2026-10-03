// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

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
  targets.proxy.textContent = proxyText(status);
  return view;
}

/** The proxy text for a page: the proxy, or a dash while it is not known. */
export function proxyText(status: { proxy: string }): string {
  return status.proxy || NO_PROXY;
}
