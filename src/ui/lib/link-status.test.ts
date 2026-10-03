// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { linkStatus, pillSize } from "./link-status.ts";

describe("[ipc-contract events] link-hover bubble", () => {
  it("[ipc-contract events] empty text hides the bubble", () => {
    assert.equal(linkStatus({ text: "", blocked: false }).visible, false);
    assert.equal(linkStatus(null).visible, false);
  });
  it("[ipc-contract events] a link shows its text", () => {
    const view = linkStatus({ text: "http://notbob.i2p/", blocked: false });
    assert.equal(view.visible, true);
    assert.equal(new URL(view.text).hostname, "notbob.i2p");
  });
  it("[ipc-contract events] blocked: true shows as blocked", () => {
    const view = linkStatus({ text: "Blocked: evil.com", blocked: true });
    assert.equal(view.visible, true);
    const shown = `${view.prefix ?? ""} ${view.text}`;
    assert.match(shown, /Blocked/);
    assert.equal(shown.split(/\s+/).at(-1), "evil.com");
  });
  it("[ipc-contract events] a link that is not blocked does not show as blocked", () => {
    const view = linkStatus({ text: "http://notbob.i2p/", blocked: false });
    assert.equal(/Blocked/.test(`${view.prefix ?? ""} ${view.text}`), false);
  });
  it("[ipc-contract events] shows a text of about 80 characters in full", () => {
    const text = `http://notbob.i2p/${"a".repeat(62)}`;
    assert.equal(text.length, 80);
    assert.equal(linkStatus({ text, blocked: false }).text, text);
  });
});

describe("[ipc-contract events] status-size", () => {
  it("[ipc-contract events] never reports less than the pill needs", () => {
    const size = pillSize({ width: 90.2, height: 19.01 });
    assert.ok(size.width >= 90.2 && size.height >= 19.01);
    assert.ok(Number.isInteger(size.width) && Number.isInteger(size.height));
  });
});
