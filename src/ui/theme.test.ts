import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { isThemePref } from "./theme.ts";

describe("isThemePref", () => {
  it("accepts supported theme preferences", () => {
    for (const value of ["system", "light", "dark"]) {
      assert.equal(isThemePref(value), true);
    }
  });

  it("rejects unsupported values", () => {
    for (const value of ["Dark", "", null, undefined, 1, {}]) {
      assert.equal(isThemePref(value), false);
    }
  });
});
