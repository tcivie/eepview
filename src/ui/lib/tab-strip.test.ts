// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { zoomText } from "./tab-strip.ts";

describe("zoom", () => {
  it("shows the zoom factor as a percentage", () => {
    assert.equal(zoomText(1), "100%");
    assert.equal(zoomText(1.1), "110%");
  });
});
