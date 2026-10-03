// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  isStep,
  percentOf,
  progressText,
  type Step,
  stepPosition,
  stepperHopState,
  totalProgress,
} from "./setup-flow.ts";

describe("download progress", () => {
  it("adds the transfers into one total", () => {
    const total = totalProgress([
      { doneMb: 31, totalMb: 31 },
      { doneMb: 11, totalMb: 44 },
    ]);
    assert.deepEqual(total, { doneMb: 42, totalMb: 75 });
  });
});

const STEP_TABLE: [Step, number][] = [
  ["install", 1],
  ["found", 1],
  ["download", 2],
  ["download-failed", 2],
  ["tunnels", 3],
  ["tunnels-failed", 3],
  ["ready", 4],
];

describe("setup step header", () => {
  for (const [step, number] of STEP_TABLE) {
    it(`Setup flow: ${step} shows "Step ${number} of 4"`, () => {
      assert.equal(stepPosition(step)?.text, `Step ${number} of 4`);
    });
  }
});

describe("setup hop strip", () => {
  const refusedHops = (step: Step): number[] => {
    const position = stepPosition(step);
    if (!position) return [];
    return [0, 1, 2, 3, 4].filter((hop) => stepperHopState(hop, position) === "refused");
  };
  it("Setup flow: turns the hop of the failed step red", () => {
    assert.equal(refusedHops("download-failed").length, 1);
    assert.equal(refusedHops("tunnels-failed").length, 1);
  });
  it("Setup flow: turns the next hop red when a later step fails", () => {
    const [download] = refusedHops("download-failed");
    const [tunnels] = refusedHops("tunnels-failed");
    assert.equal((tunnels ?? 0) - (download ?? 0), 1);
  });
  it("Setup flow: turns no hop red on a screen that has not failed", () => {
    for (const step of ["install", "found", "download", "tunnels", "ready"] as const) {
      assert.deepEqual(refusedHops(step), [], step);
    }
  });
});

describe("setup download screen", () => {
  it("Setup flow: shows MB and % for one file", () => {
    const text = progressText({ doneMb: 11, totalMb: 44 });
    assert.match(text, /11/);
    assert.match(text, /44/);
    assert.match(text, /MB/);
    assert.match(text, /25\s?%/);
    assert.equal(percentOf({ doneMb: 11, totalMb: 44 }), 25);
  });
  it("Setup flow: shows MB and % for the total", () => {
    const total = totalProgress([
      { doneMb: 20, totalMb: 40 },
      { doneMb: 20, totalMb: 40 },
    ]);
    assert.match(progressText(total), /40/);
    assert.match(progressText(total), /80/);
    assert.match(progressText(total), /50\s?%/);
  });
});

describe("setup screens", () => {
  it("Setup flow: opens a screen for each step in the step table and no other", () => {
    for (const [step] of STEP_TABLE) assert.equal(isStep(step), true, step);
    assert.equal(isStep("bogus"), false);
    assert.equal(isStep(undefined), false);
  });
});
