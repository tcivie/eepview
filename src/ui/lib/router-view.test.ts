// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { versionText } from "./router-view.ts";

describe("versionText", () => {
  it("[browser-shell ux1 6] shows I2P and the version when the router reports it", () => {
    assert.equal(versionText({ version: "2.10.0" }), "I2P 2.10.0");
  });
  it("[browser-shell ux1 6] says the version is not reported, never Unknown", () => {
    const text = versionText({ version: null });
    assert.equal(text, "Version not reported by the router");
    assert.ok(!/unknown/i.test(text));
  });
});
