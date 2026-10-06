// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT
import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { loadFailedView } from "./load-failed.ts";

const query = (pairs: Record<string, string>) => `?${new URLSearchParams(pairs).toString()}`;

describe("loadFailedView reads the page query", () => {
  it("[browser-shell F7] reads the address, the reason and the code", () => {
    const view = loadFailedView(
      query({ url: "http://a.i2p/x", reason: "blocked", code: "NSURLErrorDomain -1022" }),
    );
    assert.equal(view.address, "http://a.i2p/x");
    assert.equal(view.reason, "blocked");
    assert.equal(view.code, "NSURLErrorDomain -1022");
  });
  it("[browser-shell F7] reads a query without the leading question mark", () => {
    const view = loadFailedView("url=http%3A%2F%2Fa.i2p%2F&reason=unreachable&code=x%201");
    assert.equal(view.address, "http://a.i2p/");
    assert.equal(view.reason, "unreachable");
    assert.equal(view.code, "x 1");
  });
  it("[browser-shell F7] keeps the three known reasons", () => {
    for (const reason of ["blocked", "unreachable", "engine"] as const) {
      assert.equal(loadFailedView(query({ url: "http://a.i2p/", reason })).reason, reason);
    }
  });
});

describe("loadFailedView defaults", () => {
  it("[browser-shell F7] an unknown reason is engine", () => {
    assert.equal(loadFailedView(query({ url: "http://a.i2p/", reason: "weird" })).reason, "engine");
  });
  it("[browser-shell F7] a missing reason is engine", () => {
    assert.equal(loadFailedView(query({ url: "http://a.i2p/" })).reason, "engine");
  });
  it("[browser-shell F7] a reason in another case is engine", () => {
    assert.equal(loadFailedView(query({ reason: "BLOCKED" })).reason, "engine");
  });
  it("[browser-shell F7] a missing address is empty", () => {
    assert.equal(loadFailedView(query({ reason: "blocked" })).address, "");
  });
  it("[browser-shell F7] a missing code is empty", () => {
    assert.equal(loadFailedView(query({ url: "http://a.i2p/" })).code, "");
  });
  it("[browser-shell F7] an empty query gives an empty address and code, and engine", () => {
    for (const search of ["", "?"]) {
      const view = loadFailedView(search);
      assert.equal(view.address, "", search);
      assert.equal(view.code, "", search);
      assert.equal(view.reason, "engine", search);
    }
  });
});

describe("loadFailedView address length", () => {
  it("[browser-shell F7] an address of 2048 characters is kept whole", () => {
    const address = `http://a.i2p/${"a".repeat(2048 - 13)}`;
    assert.equal(address.length, 2048);
    assert.equal(loadFailedView(query({ url: address })).address, address);
  });
  it("[browser-shell F7] a longer address is cut to 2048 characters with an ellipsis", () => {
    const address = `http://a.i2p/${"b".repeat(5000)}`;
    const shown = loadFailedView(query({ url: address })).address;
    assert.equal(shown.length, 2048);
    assert.ok(shown.endsWith("…"));
    assert.ok(address.startsWith(shown.slice(0, -1)));
  });
  it("[browser-shell F7] an address one character over the limit is cut", () => {
    const address = "c".repeat(2049);
    const shown = loadFailedView(query({ url: address })).address;
    assert.equal(shown.length, 2048);
    assert.ok(shown.endsWith("…"));
  });
});

describe("loadFailedView reason text", () => {
  const text = (reason: string) =>
    loadFailedView(query({ url: "http://a.i2p/", reason })).reasonText;
  it("[browser-shell F7] every reason has a non-empty sentence", () => {
    for (const reason of ["blocked", "unreachable", "engine", "weird"]) {
      assert.ok(text(reason).trim().length > 0, reason);
    }
  });
  it("[browser-shell F7] the three reasons give three different sentences", () => {
    const texts = new Set(["blocked", "unreachable", "engine"].map(text));
    assert.equal(texts.size, 3);
  });
  it("[browser-shell F7] an unknown reason reads like the engine reason", () => {
    assert.equal(text("weird"), text("engine"));
  });
});
