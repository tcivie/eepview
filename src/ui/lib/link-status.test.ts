import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  BLOCKED_PREFIX,
  cleanStatusText,
  linkStatus,
  MAX_STATUS_CHARS,
  pillSize,
} from "./link-status.ts";

describe("linkStatus", () => {
  it("hides for empty or missing text", () => {
    assert.deepEqual(linkStatus(null), { visible: false, prefix: null, text: "" });
    assert.equal(linkStatus({ text: "   ", blocked: true }).visible, false);
  });
  it("shows an allowed link without a prefix", () => {
    assert.deepEqual(linkStatus({ text: "http://stats.i2p/", blocked: false }), {
      visible: true,
      prefix: null,
      text: "http://stats.i2p/",
    });
  });
  it("marks a blocked link", () => {
    assert.equal(
      linkStatus({ text: "https://example.com/", blocked: true }).prefix,
      BLOCKED_PREFIX,
    );
  });
});

describe("cleanStatusText", () => {
  it("keeps the text on one line", () => {
    assert.equal(cleanStatusText(" a\n  b\tc "), "a b c");
  });
  it("caps very long text", () => {
    const long = "x".repeat(MAX_STATUS_CHARS + 50);
    const cleaned = cleanStatusText(long);
    assert.equal(cleaned.length, MAX_STATUS_CHARS);
    assert.equal(cleaned.endsWith("…"), true);
  });
});

describe("pillSize", () => {
  it("rounds up so the pill is never cut", () => {
    assert.deepEqual(pillSize({ width: 90.2, height: 19.01 }), { width: 91, height: 20 });
    assert.deepEqual(pillSize({ width: 90, height: 20 }), { width: 90, height: 20 });
  });
});
