// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { type RouterLike, routerView } from "./router-view.ts";

const router = (state: RouterLike["state"]): RouterLike => ({
  state,
  proxy: "127.0.0.1:4444",
  version: "2.10.0",
  detail: null,
});

describe("router state", () => {
  it("Router panel: ready, building and stopped have different colors", () => {
    const tones = [router("ok"), router("building"), router("down")].map((r) => routerView(r).tone);
    assert.deepEqual(tones, ["ready", "building", "stopped"]);
  });
  it("Router panel: a ready router lets you browse and a stopped one does not", () => {
    assert.equal(routerView(router("ok")).canBrowse, true);
    assert.equal(routerView(router("down")).canBrowse, false);
  });
});
