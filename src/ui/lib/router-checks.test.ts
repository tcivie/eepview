// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// The four router checks as rows of the Home card (docs/wiki/router-checks.md, V15 to V18):
// the pure module that turns `RouterStatus.checks` into rows and announcements.

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { CheckId, CheckState, VerifyCheck } from "../contract.ts";
import { CHECK_NAMES, checkAnnouncement, checkViews, clockTime } from "./router-checks.ts";

const IDS: CheckId[] = ["proxy-i2p", "version", "no-outproxy", "tunnels"];
const NAMES = [
  "Proxy is an I2P router",
  "Router version is supported",
  "No outproxy",
  "Network up, client tunnel built",
];
const STATES: CheckState[] = ["pending", "running", "passed", "failed", "not-checked"];
const STATE_TEXT: Record<CheckState, string> = {
  pending: "Waiting",
  running: "Checking",
  passed: "Passed",
  failed: "Failed",
  "not-checked": "Not checked",
};

type Patch = Partial<Record<CheckId, Partial<VerifyCheck>>>;

/** Four checks in the order of V1, all pending, with the given changes. */
function checks(patch: Patch = {}): VerifyCheck[] {
  return IDS.map((id) => ({ id, state: "pending", detail: null, passedAt: null, ...patch[id] }));
}

const pad = (n: number): string => String(n).padStart(2, "0");
const localClock = (ms: number): string => {
  const date = new Date(ms);
  return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
};
const AT = new Date(2026, 9, 6, 13, 4, 9).getTime();

describe("V15: the four rows", () => {
  it("V15: CHECK_NAMES holds the names of the table", () => {
    assert.deepEqual(
      IDS.map((id) => CHECK_NAMES[id]),
      NAMES,
    );
  });

  it("V15: checkViews gives four rows in the order of V1 with the name, state and detail", () => {
    const views = checkViews(
      checks({
        "proxy-i2p": { state: "failed", detail: "connection refused" },
        version: { state: "not-checked", detail: "no router console found" },
      }),
    );
    assert.equal(views.length, 4);
    assert.deepEqual(
      views.map((v) => v.id),
      IDS,
    );
    assert.deepEqual(
      views.map((v) => v.name),
      NAMES,
    );
    assert.deepEqual(
      views.map((v) => v.state),
      ["failed", "not-checked", "pending", "pending"],
    );
    assert.equal(views[0]?.detail, "connection refused");
    assert.equal(views[1]?.detail, "no router console found");
    assert.equal(views[2]?.detail, null);
  });
});

describe("V16: the state as text", () => {
  it("V16: pending, running, failed and not-checked have a fixed text", () => {
    for (const state of STATES.filter((s) => s !== "passed")) {
      const [view] = checkViews(checks({ "proxy-i2p": { state, detail: "why" } }));
      assert.equal(view?.stateText, STATE_TEXT[state], state);
    }
  });

  it("V16: a passed check shows 'Passed at HH:MM:SS' in 24-hour local time of passedAt", () => {
    const [view] = checkViews(checks({ "proxy-i2p": { state: "passed", passedAt: AT } }));
    assert.equal(view?.stateText, `Passed at ${localClock(AT)}`);
    assert.equal(view?.stateText, "Passed at 13:04:09");
  });

  it("V16: a passed check with passedAt null shows 'Passed'", () => {
    const [view] = checkViews(checks({ "proxy-i2p": { state: "passed", passedAt: null } }));
    assert.equal(view?.stateText, "Passed");
  });

  it("V16: clockTime writes two digits for each part", () => {
    assert.equal(clockTime(new Date(2026, 0, 2, 3, 4, 5).getTime()), "03:04:05");
    assert.equal(clockTime(new Date(2026, 5, 7, 0, 0, 0).getTime()), "00:00:00");
  });

  it("V16: clockTime is a 24-hour clock in local time", () => {
    assert.equal(clockTime(new Date(2026, 11, 31, 23, 59, 58).getTime()), "23:59:58");
    assert.equal(clockTime(new Date(2026, 6, 1, 12, 30, 0).getTime()), "12:30:00");
    for (const ms of [0, AT, 86_400_000 * 20 + 12_345_678]) {
      assert.equal(clockTime(ms), localClock(ms), String(ms));
    }
  });
});

describe("V17: only real state", () => {
  it("V17: a missing list gives four Waiting rows", () => {
    for (const none of [null, undefined, []]) {
      const views = checkViews(none);
      assert.equal(views.length, 4);
      assert.deepEqual(
        views.map((v) => v.id),
        IDS,
      );
      for (const view of views) {
        assert.equal(view.state, "pending");
        assert.equal(view.stateText, "Waiting");
        assert.equal(view.detail, null);
      }
    }
  });

  it("V17: a short list gives four rows and the missing ones wait", () => {
    const short: VerifyCheck[] = [
      { id: "proxy-i2p", state: "failed", detail: "x", passedAt: null },
    ];
    const views = checkViews(short);
    assert.equal(views.length, 4);
    assert.deepEqual(
      views.map((v) => v.id),
      IDS,
    );
    for (const view of views.slice(1)) {
      assert.equal(view.stateText, "Waiting", view.id);
    }
  });
});

describe("V17: only real state, rows", () => {
  it("V17: a row is passed only when its check is passed", () => {
    for (const state of STATES) {
      const views = checkViews(checks({ tunnels: { state, detail: "d", passedAt: AT } }));
      const passed = views.filter((v) => v.state === "passed").map((v) => v.id);
      assert.deepEqual(passed, state === "passed" ? ["tunnels"] : [], state);
    }
  });

  it("V17: the same checks give the same rows (no clock decides a row)", () => {
    const list = checks({ "proxy-i2p": { state: "passed", passedAt: AT } });
    assert.deepEqual(checkViews(list), checkViews(list));
  });
});

describe("V18: announcements", () => {
  it("V18: nothing on the first render", () => {
    for (const before of [null, undefined]) {
      const after = checks({ "proxy-i2p": { state: "passed", passedAt: AT } });
      assert.equal(checkAnnouncement(before, after), "");
    }
  });

  it("V18: nothing when nothing changed", () => {
    const list = checks({ "proxy-i2p": { state: "passed", passedAt: AT } });
    assert.equal(checkAnnouncement(list, structuredClone(list)), "");
    assert.equal(checkAnnouncement(null, null), "");
  });

  it("V18: nothing when only a detail changed", () => {
    const before = checks({ version: { state: "passed", detail: "2.4.0 is at least 2.4.0" } });
    const after = checks({ version: { state: "passed", detail: "2.5.0 is at least 2.4.0" } });
    assert.equal(checkAnnouncement(before, after), "");
  });

  it("V18: nothing when only a passedAt changed", () => {
    const before = checks({ "proxy-i2p": { state: "passed", passedAt: AT } });
    const after = checks({ "proxy-i2p": { state: "passed", passedAt: AT + 60_000 } });
    assert.equal(checkAnnouncement(before, after), "");
  });
});

describe("V18: announcements of a change", () => {
  it("V18: one change says '<name>: <state text>.'", () => {
    const before = checks({ "proxy-i2p": { state: "passed", passedAt: AT } });
    const after = checks({ "proxy-i2p": { state: "failed", detail: "connection refused" } });
    assert.equal(checkAnnouncement(before, after), "Proxy is an I2P router: Failed.");
  });

  it("V18: a check that turns passed is announced with the pass time", () => {
    const before = checks({ "proxy-i2p": { state: "running" } });
    const after = checks({ "proxy-i2p": { state: "passed", passedAt: AT } });
    assert.equal(
      checkAnnouncement(before, after),
      `Proxy is an I2P router: Passed at ${localClock(AT)}.`,
    );
  });

  it("V18: each state is announced with its own text", () => {
    for (const state of STATES.filter((s) => s !== "passed" && s !== "pending")) {
      const after = checks({ tunnels: { state, detail: "d" } });
      const said = checkAnnouncement(checks(), after);
      assert.equal(said, `Network up, client tunnel built: ${STATE_TEXT[state]}.`, state);
    }
    const back = checkAnnouncement(checks({ tunnels: { state: "failed", detail: "d" } }), checks());
    assert.equal(back, "Network up, client tunnel built: Waiting.");
  });
});

describe("V18: announcements of several changes", () => {
  it("V18: several changes are one sentence each, in check order", () => {
    const before = checks({
      "proxy-i2p": { state: "passed", passedAt: AT },
      version: { state: "passed", passedAt: AT },
      tunnels: { state: "passed", passedAt: AT },
    });
    const after = checks({ "proxy-i2p": { state: "failed", detail: "refused" } });
    const said = checkAnnouncement(before, after);
    const parts = [
      "Proxy is an I2P router: Failed.",
      "Router version is supported: Waiting.",
      "Network up, client tunnel built: Waiting.",
    ];
    let from = 0;
    for (const part of parts) {
      const at = said.indexOf(part, from);
      assert.ok(at >= 0, `${part} in order in: ${said}`);
      from = at + part.length;
    }
    assert.ok(!said.includes("No outproxy"), "V18: an unchanged check is not announced");
  });
});
