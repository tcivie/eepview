// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { type PanelStatusLike, panelControls } from "./router-panel.ts";

const status = (patch: Partial<PanelStatusLike> = {}): PanelStatusLike => ({
  state: "ok",
  proxy: "127.0.0.1:4444",
  version: "2.10.0",
  detail: null,
  managed: true,
  paused: false,
  ...patch,
});

describe("panelControls", () => {
  it("offers pause while browsing and resume while paused", () => {
    assert.equal(panelControls(status()).pauseCommand, "connection_pause");
    assert.equal(panelControls(status()).pauseLabel, "Pause I2P browsing");
    assert.equal(panelControls(status({ paused: true })).pauseCommand, "connection_resume");
  });
});
