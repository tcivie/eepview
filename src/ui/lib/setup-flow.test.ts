// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { totalProgress } from "./setup-flow.ts";

describe("download progress", () => {
  it("adds the transfers into one total", () => {
    const total = totalProgress([
      { doneMb: 31, totalMb: 31 },
      { doneMb: 11, totalMb: 44 },
    ]);
    assert.deepEqual(total, { doneMb: 42, totalMb: 75 });
  });
});
