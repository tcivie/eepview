// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// Requirement tests R11.1 and R11.2 of docs/wiki/diagnostics-and-bug-reports.md.
import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  BROWSER_NOTE,
  CRASH_TEXT,
  DRAG_NOTE,
  openedMessage,
  PLACEHOLDER,
  reportHref,
  reportKind,
} from "./report-page.ts";

const KINDS = ["general", "crash", "blocked", "router-down", "load-failed"] as const;

describe("texts of the report page (R11.1, R3.5)", () => {
  it("R11.1: the description placeholder", () => {
    assert.equal(
      PLACEHOLDER,
      "What did you do, and what went wrong? Do not include site addresses.",
    );
  });
  it("R11.1: the note says GitHub opens outside I2P and sees the IP address", () => {
    assert.equal(
      BROWSER_NOTE,
      "This opens GitHub in your normal web browser, outside I2P. GitHub sees your IP address and your GitHub account. Nothing is sent until you submit the issue on GitHub.",
    );
    assert.match(BROWSER_NOTE, /GitHub sees your IP address/);
  });
  it("R11.1: the note after the click tells the user to drag the file", () => {
    assert.equal(DRAG_NOTE, "Drag this file into the GitHub issue to attach it.");
  });
  it("R3.5: the crash banner text", () => {
    assert.equal(CRASH_TEXT, "eepview closed unexpectedly. Report the problem?");
  });
});

describe("reportKind (R11.2)", () => {
  it("reads the kind from the query string, with or without the question mark", () => {
    for (const kind of KINDS) {
      assert.equal(reportKind(`?kind=${kind}`), kind);
      assert.equal(reportKind(`kind=${kind}`), kind);
    }
  });
  it("finds the kind among other parameters", () => {
    assert.equal(reportKind("?x=1&kind=crash"), "crash");
    assert.equal(reportKind("?kind=blocked&x=1"), "blocked");
  });
  it("falls back to general for no kind, an empty kind or an unknown kind", () => {
    for (const search of ["", "?", "?kind=", "?kind=bogus", "?other=crash", "?kind=<script>"]) {
      assert.equal(reportKind(search), "general", search);
    }
  });
});

describe("reportHref (R11.2, R11.3)", () => {
  it("is ./report.html?kind=<kind>", () => {
    for (const kind of KINDS) {
      assert.equal(reportHref(kind), `./report.html?kind=${kind}`);
    }
  });
  it("carries the kind only", () => {
    assert.equal(reportHref("blocked").split("?")[1], "kind=blocked");
    assert.equal(reportHref("router-down").split("?")[1], "kind=router-down");
  });
  it("round-trips through reportKind", () => {
    for (const kind of KINDS) {
      assert.equal(reportKind(reportHref(kind).slice(reportHref(kind).indexOf("?"))), kind);
    }
  });
});

describe("openedMessage (R11.2)", () => {
  const file = "eepview-report-2026-10-03T12-00-00Z.txt";
  it("names the file and ends with the drag note", () => {
    const message = openedMessage({ file, trimmed: false });
    assert.ok(message.includes(file), message);
    assert.ok(message.endsWith(DRAG_NOTE), message);
  });
  it("says the issue has a shorter log than the file when trimmed", () => {
    const plain = openedMessage({ file, trimmed: false });
    const trimmed = openedMessage({ file, trimmed: true });
    assert.notEqual(trimmed, plain);
    assert.match(trimmed, /shorter/i);
    assert.ok(trimmed.includes(file), trimmed);
    assert.ok(trimmed.endsWith(DRAG_NOTE), trimmed);
  });
  it("does not mention a shorter log when the log was not trimmed", () => {
    assert.ok(!/shorter/i.test(openedMessage({ file, trimmed: false })));
  });
  it("shows no folder, only the file name it was given", () => {
    const message = openedMessage({ file, trimmed: false });
    assert.ok(!/\/Users\/|\/home\/|C:\\/.test(message), message);
  });
});
