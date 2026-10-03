// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { NO_PROXY, renderRouterSummary } from "./router-summary.ts";

const targets = () => ({
  chip: { textContent: "" as string | null, dataset: {} as Record<string, string | undefined> },
  text: { textContent: "" as string | null },
  proxy: { textContent: "" as string | null },
});

describe("renderRouterSummary", () => {
  it("fills the chip, the text and the proxy", () => {
    const t = targets();
    const view = renderRouterSummary(t, {
      state: "ok",
      proxy: "127.0.0.1:4444",
      version: "2.10.0",
      detail: null,
    });
    assert.equal(t.chip.dataset.tone, "ready");
    assert.equal(t.chip.textContent, "Ready");
    assert.equal(t.text.textContent, view.text);
    assert.equal(t.proxy.textContent, "127.0.0.1:4444");
  });
  it("shows a dash when there is no proxy", () => {
    const t = targets();
    renderRouterSummary(t, { state: "down", proxy: "", version: null, detail: null });
    assert.equal(t.chip.dataset.tone, "stopped");
    assert.equal(t.proxy.textContent, NO_PROXY);
  });
});
