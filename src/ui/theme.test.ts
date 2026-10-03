import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { isThemePref } from "./theme.ts";

describe("isThemePref", () => {
  it("accepts the three valid preferences", () => {
    assert.equal(isThemePref("system"), true);
    assert.equal(isThemePref("light"), true);
    assert.equal(isThemePref("dark"), true);
  });

  it("rejects a differently-cased preference", () => {
    assert.equal(isThemePref("Dark"), false);
    assert.equal(isThemePref("LIGHT"), false);
    assert.equal(isThemePref("System"), false);
  });

  it("rejects an empty string", () => {
    assert.equal(isThemePref(""), false);
  });

  it("rejects null and undefined", () => {
    assert.equal(isThemePref(null), false);
    assert.equal(isThemePref(undefined), false);
  });

  it("rejects non-strings", () => {
    assert.equal(isThemePref(42), false);
    assert.equal(isThemePref({ pref: "dark" }), false);
  });
});
