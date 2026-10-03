// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { isThemePref } from "./theme.ts";

describe("Settings.theme", () => {
  it("accepts system, light and dark", () => {
    for (const theme of ["system", "light", "dark"]) {
      assert.equal(isThemePref(theme), true, theme);
    }
  });
  it("refuses any other value", () => {
    for (const theme of ["blue", "", "Dark", null, undefined, 1]) {
      assert.equal(isThemePref(theme), false, String(theme));
    }
  });
});
