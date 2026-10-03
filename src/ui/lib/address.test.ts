import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  displayUrl,
  eepsiteUrl,
  hostOf,
  internalPageOf,
  internalUrlForFile,
  isI2pAddress,
  isInternal,
} from "./address.ts";

describe("hostOf", () => {
  it("strips the scheme, the path and the case", () => {
    assert.equal(hostOf("HTTP://Stats.I2P/cgi-bin/x?y#z"), "stats.i2p");
  });
  it("accepts a bare host", () => {
    assert.equal(hostOf("  zzz.i2p "), "zzz.i2p");
  });
  it("drops the port and the user info", () => {
    assert.equal(hostOf("http://user:pw@foo.i2p:8080/x"), "foo.i2p");
    assert.equal(isI2pAddress("http://foo.i2p:8080/"), true);
  });
  it("returns empty text for empty input", () => {
    assert.equal(hostOf(""), "");
  });
});

describe("isI2pAddress", () => {
  it("accepts .i2p and .b32.i2p hosts", () => {
    assert.equal(isI2pAddress("notbob.i2p"), true);
    assert.equal(isI2pAddress("https://abc234.b32.i2p/path"), true);
  });
  it("refuses clearnet and lookalike hosts", () => {
    assert.equal(isI2pAddress("example.com"), false);
    assert.equal(isI2pAddress("i2p.example.com"), false);
    assert.equal(isI2pAddress("evil.i2p.com"), false);
    assert.equal(isI2pAddress("-bad.i2p"), false);
  });
});

describe("internal addresses", () => {
  it("detects eepview urls", () => {
    assert.equal(isInternal("eepview://home"), true);
    assert.equal(isInternal("http://home.i2p/"), false);
  });
  it("finds the page name", () => {
    assert.equal(internalPageOf("eepview://history?q=forum"), "history");
    assert.equal(internalPageOf("eepview://"), null);
    assert.equal(internalPageOf("http://stats.i2p/"), null);
  });
  it("maps a bundled file link to an eepview url", () => {
    assert.equal(internalUrlForFile("./bookmarks.html"), "eepview://bookmarks");
    assert.equal(internalUrlForFile("/src/ui/history.html?q=a#x"), "eepview://history?q=a");
    assert.equal(internalUrlForFile("http://stats.i2p/"), null);
    assert.equal(internalUrlForFile("http://foo.i2p:8080/index.html"), null);
  });
});

describe("eepsiteUrl", () => {
  it("adds http to a bare host", () => {
    assert.equal(eepsiteUrl("stats.i2p"), "http://stats.i2p/");
  });
  it("keeps an existing scheme", () => {
    assert.equal(eepsiteUrl("https://zzz.i2p/topics"), "https://zzz.i2p/topics");
  });
});

describe("displayUrl", () => {
  it("keeps internal urls as they are", () => {
    assert.equal(displayUrl("eepview://settings"), "eepview://settings");
  });
  it("hides the scheme and a lone trailing slash", () => {
    assert.equal(displayUrl("http://stats.i2p/"), "stats.i2p");
    assert.equal(displayUrl("http://notbob.i2p/uptime"), "notbob.i2p/uptime");
    assert.equal(displayUrl("http://notbob.i2p/a/"), "notbob.i2p/a/");
  });
});
