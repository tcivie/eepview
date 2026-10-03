import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  countText,
  formatElapsed,
  isStep,
  percentOf,
  progressText,
  STEP_TOTAL,
  type StepPosition,
  stepPosition,
  stepperHopState,
  totalProgress,
} from "./setup-flow.ts";

function position(step: Parameters<typeof stepPosition>[0]): StepPosition {
  const found = stepPosition(step);
  if (!found) throw new Error(`no position for ${step}`);
  return found;
}

describe("isStep", () => {
  it("accepts every screen and rejects anything else", () => {
    assert.equal(isStep("tunnels-failed"), true);
    assert.equal(isStep("later"), true);
    assert.equal(isStep("toString"), false);
    assert.equal(isStep(3), false);
  });
});

describe("stepPosition", () => {
  it("numbers the four steps from one", () => {
    assert.equal(position("install").text, `Step 1 of ${STEP_TOTAL}`);
    assert.equal(position("download").text, "Step 2 of 4");
    assert.equal(position("ready").text, "Step 4 of 4");
  });
  it("puts a found router and a failure on the step they replace", () => {
    assert.equal(position("found").index, 0);
    assert.deepEqual(
      [position("tunnels-failed").index, position("tunnels-failed").failed],
      [2, true],
    );
  });
  it("has no position for the not-now screen", () => {
    assert.equal(stepPosition("later"), null);
  });
});

describe("stepperHopState", () => {
  it("marks earlier hops built, the current one building and later ones empty", () => {
    const states = [0, 1, 2, 3].map((hop) => stepperHopState(hop, position("download")));
    assert.deepEqual(states, ["built", "building", null, null]);
  });
  it("marks the current hop refused on a failure", () => {
    assert.equal(stepperHopState(1, position("download-failed")), "refused");
  });
  it("marks every hop built once ready", () => {
    const states = [0, 1, 2, 3].map((hop) => stepperHopState(hop, position("ready")));
    assert.deepEqual(states, ["built", "built", "built", "built"]);
  });
});

describe("download progress", () => {
  it("shows megabytes and a whole percent", () => {
    assert.equal(progressText({ doneMb: 19.4, totalMb: 31 }), "19.4 of 31 MB · 62%");
  });
  it("never passes 100 percent and copes with an empty total", () => {
    assert.equal(percentOf({ doneMb: 40, totalMb: 31 }), 100);
    assert.equal(percentOf({ doneMb: 0, totalMb: 0 }), 0);
  });
  it("adds the transfers into one total", () => {
    const total = totalProgress([
      { doneMb: 31, totalMb: 31 },
      { doneMb: 11, totalMb: 44 },
    ]);
    assert.deepEqual(total, { doneMb: 42, totalMb: 75 });
  });
});

describe("formatElapsed", () => {
  it("shows minutes only when there are any", () => {
    assert.equal(formatElapsed(100), "1 min 40 s");
    assert.equal(formatElapsed(42), "42 s");
  });
});

describe("countText", () => {
  it("reads as done of total", () => {
    assert.equal(countText(1, 2), "1 of 2");
  });
});
