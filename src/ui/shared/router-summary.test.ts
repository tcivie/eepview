// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { NO_PROXY, proxyText } from "./router-summary.ts";

describe("proxyText", () => {
  it("[browser-shell ux1 5] returns the proxy eepview uses", () => {
    assert.equal(proxyText({ proxy: "127.0.0.1:4444" }), "127.0.0.1:4444");
    assert.equal(proxyText({ proxy: "10.0.0.5:5555" }), "10.0.0.5:5555");
  });
  it("[browser-shell ux1 5] returns the dash while the proxy is not known", () => {
    assert.equal(proxyText({ proxy: "" }), NO_PROXY);
    assert.equal(NO_PROXY, "—");
  });
});
