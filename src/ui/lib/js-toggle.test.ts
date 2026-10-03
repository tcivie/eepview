import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { jsToggleLabel } from "./js-toggle.ts";

describe("jsToggleLabel", () => {
  it("names the state and the host", () => {
    assert.equal(jsToggleLabel(true, "forum.i2p"), "JavaScript on for forum.i2p");
    assert.equal(jsToggleLabel(false, "forum.i2p"), "JavaScript off for forum.i2p");
  });
  it("drops the host when there is none", () => {
    assert.equal(jsToggleLabel(false, ""), "JavaScript off");
  });
});
