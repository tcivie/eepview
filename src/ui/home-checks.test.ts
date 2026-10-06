// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// The router card of the Home page on a stand-in shell (docs/wiki/router-checks.md, V15 to V19):
// the four check rows show only what the backend reports, and nothing else changes a row.

import assert from "node:assert/strict";
import { after, describe, it } from "node:test";
import type { CheckId, VerifyCheck } from "./contract.ts";
import { sleep, stubShell, until } from "./testing/shell-stub.ts";

const stub = stubShell("home.html");
const { doc } = stub;

const IDS: CheckId[] = ["proxy-i2p", "version", "no-outproxy", "tunnels"];
const NAMES = [
  "Proxy is an I2P router",
  "Router version is supported",
  "No outproxy",
  "Network up, client tunnel built",
];
const AT = new Date(2026, 9, 6, 13, 4, 9).getTime();
const NO_CONSOLE = { found: false, kind: null, origin: null, version: null };

const row = (id: CheckId, state: VerifyCheck["state"], detail: string | null): VerifyCheck => ({
  id,
  state,
  detail,
  passedAt: state === "passed" ? AT : null,
});

// Check 1 passed, 2 failed, 3 not checked, 4 pending: one row of each kind of text.
const FIRST: VerifyCheck[] = [
  row("proxy-i2p", "passed", "proxy.i2p answered"),
  row("version", "failed", "Version 2.3.0 is older than the minimum 2.4.0"),
  row("no-outproxy", "not-checked", "No router console found, so the router type is unknown"),
  row("tunnels", "pending", null),
];

const base = {
  state: "ok",
  proxy: "127.0.0.1:4444",
  version: "2.50.1",
  detail: null,
  paused: false,
  managed: false,
};
let current: object = { ...base, checks: FIRST };

stub.answer("router_status", () => current);
stub.answer("console_status", () => NO_CONSOLE);
stub.answer("console_detect", () => NO_CONSOLE);
stub.answer("bookmarks_list", () => []);
stub.answer("diag_crash_status", () => false);
await import("./home.ts");

const rows = (): HTMLElement[] =>
  Array.from(doc.querySelectorAll<HTMLElement>("ol#router-checks > li.check"));
const at = (index: number): HTMLElement => {
  const found = rows()[index];
  if (!found) throw new Error(`no row ${index}`);
  return found;
};
const part = (item: HTMLElement, selector: string): string =>
  item.querySelector(selector)?.textContent?.trim() ?? "";
const states = (): (string | null)[] => rows().map((r) => r.getAttribute("data-state"));
const live = (): string => doc.getElementById("router-checks-live")?.textContent?.trim() ?? "";
const pad = (n: number): string => String(n).padStart(2, "0");
const clock = (ms: number): string => {
  const d = new Date(ms);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
};

/** Sends a `router-status` event with these checks and lets the page draw it. */
async function send(checks: VerifyCheck[] | undefined): Promise<void> {
  current = { ...base, checks };
  stub.fire("router-status", current);
  await sleep(80);
}

after(() => stub.window.happyDOM.close());

describe("V15: the four rows", () => {
  it("V15: the card draws four rows from the router_status answer", async () => {
    assert.ok(await until(() => states().includes("passed")), "the page drew the checks");
    assert.equal(rows().length, 4);
    assert.deepEqual(
      rows().map((r) => r.getAttribute("data-check")),
      IDS,
    );
  });

  it('V15: the list is an ordered list labelled "Router checks"', () => {
    const list = doc.getElementById("router-checks");
    assert.equal(list?.tagName.toLowerCase(), "ol");
    assert.equal(list?.getAttribute("aria-label"), "Router checks");
  });

  it("V15: each row shows the name of its check", () => {
    assert.deepEqual(
      rows().map((r) => part(r, ".check-name")),
      NAMES,
    );
  });

  it("V15: each row shows its detail when it has one", () => {
    assert.ok(part(at(0), ".check-detail").includes("proxy.i2p answered"));
    assert.ok(part(at(1), ".check-detail").includes("2.3.0"));
    assert.ok(part(at(2), ".check-detail").includes("No router console found"));
    assert.equal(part(at(3), ".check-detail"), "", "a pending check has none");
  });

  it("V15: the data-state of a row is the state of its check", () => {
    assert.deepEqual(states(), ["passed", "failed", "not-checked", "pending"]);
  });
});

describe("V18: the first render", () => {
  it("V18: the live region says nothing on the first render", () => {
    assert.equal(live(), "");
  });
});

describe("V16: the state as text", () => {
  it("V16: Passed at HH:MM:SS, Failed, Not checked and Waiting", () => {
    assert.deepEqual(
      rows().map((r) => part(r, ".check-state")),
      [`Passed at ${clock(AT)}`, "Failed", "Not checked", "Waiting"],
    );
  });

  it("V16: a running check shows Checking", async () => {
    await send([...FIRST.slice(0, 3), row("tunnels", "running", null)]);
    assert.equal(part(at(3), ".check-state"), "Checking");
    assert.equal(rows()[3]?.getAttribute("data-state"), "running");
  });

  it("V16: a passed check with no passedAt shows Passed", async () => {
    const odd: VerifyCheck = { id: "tunnels", state: "passed", detail: null, passedAt: null };
    await send([...FIRST.slice(0, 3), odd]);
    assert.equal(part(at(3), ".check-state"), "Passed");
  });
});

describe("V17: only real state", () => {
  it("V17: a row is passed only when the fired status says passed", async () => {
    await send(FIRST);
    assert.deepEqual(states(), ["passed", "failed", "not-checked", "pending"]);
    await send(IDS.map((id) => row(id, "failed", "no")));
    assert.ok(!states().includes("passed"), "no row looks passed when no check passed");
    await send(IDS.map((id) => row(id, "passed", null)));
    assert.deepEqual(states(), ["passed", "passed", "passed", "passed"]);
  });

  it("V17: a status with no checks shows four rows Waiting", async () => {
    await send(undefined);
    assert.equal(rows().length, 4);
    assert.deepEqual(
      rows().map((r) => part(r, ".check-state")),
      ["Waiting", "Waiting", "Waiting", "Waiting"],
    );
    assert.deepEqual(states(), ["pending", "pending", "pending", "pending"]);
  });

  it("V17: nothing changes a row without an event (no timer drives the rows)", async () => {
    await send(FIRST);
    const before = rows().map((r) => r.outerHTML);
    const live0 = live();
    await sleep(1500);
    assert.deepEqual(
      rows().map((r) => r.outerHTML),
      before,
    );
    assert.equal(live(), live0);
  });
});

describe("V18: announcements", () => {
  it("V18: the live region is a polite status region", () => {
    const region = doc.getElementById("router-checks-live");
    assert.ok(region, "#router-checks-live exists");
    assert.equal(region.getAttribute("role"), "status");
  });

  it("V18: a change from passed to failed changes the row and is announced", async () => {
    await send(FIRST);
    await send([row("proxy-i2p", "failed", "connection refused"), ...FIRST.slice(1)]);
    assert.equal(rows()[0]?.getAttribute("data-state"), "failed");
    assert.equal(part(at(0), ".check-state"), "Failed");
    assert.ok(live().includes("Proxy is an I2P router: Failed."), `announced: ${live()}`);
  });

  it("V18: a change of only a detail is not announced again", async () => {
    await send([row("proxy-i2p", "failed", "connection refused"), ...FIRST.slice(1)]);
    const said = live();
    await send([row("proxy-i2p", "failed", "timed out"), ...FIRST.slice(1)]);
    assert.ok(part(at(0), ".check-detail").includes("timed out"));
    assert.ok(live() === said || live() === "", `unchanged or empty, got: ${live()}`);
  });
});

describe("V19: the rest of the card", () => {
  it("V19: the state chip, the router line, the proxy and the version are still there", async () => {
    await send(FIRST);
    for (const id of ["router-chip", "router-version", "router-proxy"]) {
      assert.ok(doc.getElementById(id), `#${id} exists`);
    }
    assert.ok(doc.getElementById("router-proxy")?.textContent?.includes("127.0.0.1:4444"));
    assert.ok(doc.getElementById("router-version")?.textContent?.includes("2.50.1"));
  });

  it('V19: the "Network details" link is still there', () => {
    const links = Array.from(doc.querySelectorAll("a, button"));
    assert.ok(links.some((l) => l.textContent?.trim() === "Network details"));
  });

  it("V19: the router console section is still there", () => {
    assert.ok(doc.getElementById("console-links"), "#console-links exists");
    assert.ok(doc.getElementById("console-note"), "#console-note exists");
  });

  it("V19: the decorative hop path is gone", () => {
    assert.equal(doc.getElementById("router-hops"), null);
  });
});
