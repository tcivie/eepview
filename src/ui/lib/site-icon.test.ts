// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { ICON_PREFIX, siteMark } from "./site-icon.ts";

// A real 1 x 1 PNG.
const PNG_1X1 =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";

describe("site icon prefix (R32)", () => {
  it("is the PNG data URL prefix", () => {
    assert.equal(ICON_PREFIX, "data:image/png;base64,");
  });
});

describe("siteMark shows an icon (R32, R33)", () => {
  it("accepts a PNG data URL", () => {
    const src = `data:image/png;base64,${PNG_1X1}`;
    assert.deepEqual(siteMark(src, "A"), { kind: "icon", src });
  });
  it("accepts base64 with the characters + / and padding", () => {
    const src = "data:image/png;base64,+//+AQ==";
    assert.deepEqual(siteMark(src, "A"), { kind: "icon", src });
  });
});

describe("siteMark falls back to the letter chip (R32, R33)", () => {
  it("has no icon for null, undefined and the empty string", () => {
    assert.deepEqual(siteMark(null, "Z"), { kind: "letter", text: "Z" });
    assert.deepEqual(siteMark(undefined, "Z"), { kind: "letter", text: "Z" });
    assert.deepEqual(siteMark("", "Z"), { kind: "letter", text: "Z" });
  });
  it("keeps the letter it was given", () => {
    assert.deepEqual(siteMark(null, "Q"), { kind: "letter", text: "Q" });
    assert.deepEqual(siteMark(null, "7"), { kind: "letter", text: "7" });
  });
  it("refuses every type but PNG", () => {
    for (const type of ["svg+xml", "jpeg", "gif", "webp", "x-icon", "vnd.microsoft.icon"]) {
      const bad = `data:image/${type};base64,${PNG_1X1}`;
      assert.equal(siteMark(bad, "A").kind, "letter", type);
    }
  });
  it("refuses a data URL that is not an image", () => {
    assert.equal(siteMark("data:text/html;base64,PGgxPmhpPC9oMT4=", "A").kind, "letter");
    assert.equal(siteMark("data:text/html,<script>alert(1)</script>", "A").kind, "letter");
  });
  it("refuses http, https, file, blob and javascript URLs", () => {
    for (const bad of [
      "http://site.i2p/favicon.ico",
      "https://example.com/favicon.ico",
      "//example.com/favicon.ico",
      "file:///etc/passwd",
      "blob:http://site.i2p/1234",
      "javascript:alert(1)",
      "/favicon.ico",
      "favicon.ico",
    ]) {
      assert.equal(siteMark(bad, "A").kind, "letter", bad);
    }
  });
});

describe("siteMark refuses a bad prefix or a bad rest (R32)", () => {
  it("refuses a value that does not start with the exact prefix", () => {
    for (const bad of [
      ` data:image/png;base64,${PNG_1X1}`,
      `DATA:image/png;base64,${PNG_1X1}`,
      `data:image/PNG;base64,${PNG_1X1}`,
      `data:image/png;charset=utf-8;base64,${PNG_1X1}`,
      `data:image/png,${PNG_1X1}`,
      `data:image/png;base64${PNG_1X1}`,
      `xdata:image/png;base64,${PNG_1X1}`,
    ]) {
      assert.equal(siteMark(bad, "A").kind, "letter", bad);
    }
  });
  it("refuses a rest that is not base64", () => {
    for (const rest of [
      "AAAA<script>alert(1)</script>",
      'AAAA" onerror="alert(1)',
      "AAAA')",
      "AAAA;evil",
      "AAAA#frag",
      "AAAA?x=1",
      "AA AA",
      "AAAA\nAAAA",
      "AAAA\u0000",
      "AAAA=A",
      "http://example.com/x.png",
    ]) {
      assert.equal(siteMark(`${ICON_PREFIX}${rest}`, "A").kind, "letter", rest);
    }
  });
});
