// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { addressBadge, eepsiteUrl, internalPageOf, isI2pAddress, isInternal } from "./address.ts";

describe("isI2pAddress", () => {
  it("accepts .i2p and .b32.i2p hosts", () => {
    assert.equal(isI2pAddress("notbob.i2p"), true);
    assert.equal(isI2pAddress("https://abc234.b32.i2p/path"), true);
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

describe("[ipc-contract navigation-active-tab] only .i2p and .b32.i2p hosts load", () => {
  const lookalikes = [
    "stats.i2p.evil.com",
    "http://stats.i2p.evil.com/",
    "evil.com/stats.i2p",
    "http://evil.com/?next=stats.i2p",
    "http://stats.i2p@evil.com/",
    "https://evil.com#notbob.i2p",
    "evili2p.com",
    "https://example.com",
    "localhost:8080",
  ];
  for (const input of lookalikes) {
    it(`[ipc-contract navigation-active-tab] refuses lookalike or clearnet address ${input}`, () => {
      assert.equal(isI2pAddress(input), false);
    });
  }
  it("[ipc-contract navigation-active-tab] accepts .i2p and .b32.i2p hosts with or without a scheme", () => {
    assert.equal(isI2pAddress("stats.i2p"), true);
    assert.equal(isI2pAddress("http://notbob.i2p/"), true);
    assert.equal(isI2pAddress("https://abc234.b32.i2p/path"), true);
  });
});

describe("[ipc-contract window-layout] eepview:// pages", () => {
  it("[ipc-contract window-layout] eepview://<page>?<query> is the internal page <page>", () => {
    assert.equal(internalPageOf("eepview://history?q=notbob"), "history");
    assert.equal(internalPageOf("eepview://blocked?url=http%3A%2F%2Fevil.com"), "blocked");
    assert.equal(internalPageOf("eepview://router-down?state=down"), "router-down");
    assert.equal(internalPageOf("eepview://home"), "home");
  });
  it("[ipc-contract window-layout] an eepsite address is not an internal page", () => {
    assert.equal(isInternal("eepview://settings"), true);
    assert.equal(isInternal("http://notbob.i2p/"), false);
    assert.equal(internalPageOf("http://notbob.i2p/"), null);
  });
});

describe("[router-console R26] the badge before the address", () => {
  it("[router-console R26] a web tab shows the I2P badge, titled 'Opened over I2P'", () => {
    const badge = addressBadge({ kind: "web", url: "http://notbob.i2p/" });
    assert.deepEqual(badge, { text: "I2P", title: "Opened over I2P", kind: "i2p" });
  });

  it("[router-console R26] the console tab shows the Router console badge in place of I2P", () => {
    const badge = addressBadge({ kind: "console", url: "http://127.0.0.1:7657/home" });
    assert.deepEqual(badge, {
      text: "Router console",
      title: "The router's own console on this computer",
      kind: "console",
    });
  });

  it("[router-console R26] the console badge does not depend on the console page", () => {
    for (const url of ["http://127.0.0.1:7657/home", "http://127.0.0.1:7070/", "about:blank"]) {
      assert.equal(addressBadge({ kind: "console", url })?.kind, "console", url);
    }
  });

  it("[router-console R26] an internal page shows no badge", () => {
    assert.equal(addressBadge({ kind: "internal", url: "eepview://settings" }), null);
    assert.equal(addressBadge({ kind: "internal", url: "eepview://home" }), null);
  });

  it("[router-console R26] no tab shows no badge", () => {
    assert.equal(addressBadge(null), null);
    assert.equal(addressBadge(undefined), null);
  });
});
