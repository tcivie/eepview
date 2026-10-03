// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { hopStates, routerView } from "./router-view.ts";

const base = { proxy: "127.0.0.1:4444", version: null, detail: null };

describe("routerView", () => {
  it("is ready only when the state is ok", () => {
    const view = routerView({ ...base, state: "ok" });
    assert.equal(view.tone, "ready");
    assert.equal(view.canBrowse, true);
    assert.equal(routerView({ ...base, state: "building" }).canBrowse, false);
  });
  it("treats an outproxy and a fake proxy as stopped", () => {
    assert.equal(routerView({ ...base, state: "outproxy" }).tone, "stopped");
    assert.equal(routerView({ ...base, state: "not-i2p" }).label, "Not I2P");
  });
  it("adds the version and proxy when known", () => {
    const view = routerView({ ...base, state: "ok", version: "2.10.0" });
    assert.match(view.text, /I2P 2\.10\.0 at 127\.0\.0\.1:4444/);
  });
  it("prefers the detail from the core", () => {
    const view = routerView({ ...base, state: "down", detail: "Exit code 1." });
    assert.equal(view.text, "Exit code 1.");
  });
});

describe("hopStates", () => {
  it("builds every hop when ready", () => {
    assert.deepEqual(hopStates("ready", 3), ["built", "built", "built"]);
  });
  it("shows the next hop building", () => {
    assert.deepEqual(hopStates("building", 3), ["built", "building", null]);
  });
  it("shows the refused hop when stopped", () => {
    assert.deepEqual(hopStates("stopped", 3), ["built", "refused", null]);
  });
});
