// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { homepageValue } from "./settings-form.ts";

describe("homepage", () => {
  it("[ipc-contract navigation-active-tab] a clearnet homepage is refused like any clearnet address", () => {
    assert.equal(homepageValue("custom", "https://example.com"), null);
    assert.equal(homepageValue("custom", "example.com"), null);
  });
  it("[ipc-contract navigation-active-tab] an .i2p homepage is accepted", () => {
    assert.ok(homepageValue("custom", "notbob.i2p"));
  });
});
