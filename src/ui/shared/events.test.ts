// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  bindClicks,
  delegateClick,
  errorMessage,
  keyActions,
  type Listenable,
  runAndAnnounce,
} from "./events.ts";

class FakeTarget implements Listenable {
  listeners = new Map<string, (event: Event) => void>();
  addEventListener(type: string, listener: (event: Event) => void): void {
    this.listeners.set(type, listener);
  }
  fire(type: string, event: unknown): void {
    this.listeners.get(type)?.(event as Event);
  }
}

const element = (matches: Record<string, unknown>) => ({
  closest: (selector: string) => matches[selector] ?? null,
});

describe("delegateClick", () => {
  it("calls the handler with the closest match", () => {
    const root = new FakeTarget();
    const seen: unknown[] = [];
    delegateClick(root, "[data-id]", (match) => seen.push(match));
    root.fire("click", { target: element({ "[data-id]": "row-1" }) });
    root.fire("click", { target: element({}) });
    root.fire("click", { target: null });
    assert.deepEqual(seen, ["row-1"]);
  });
});

describe("bindClicks", () => {
  it("wires each id to its action", () => {
    const targets = new Map([
      ["a", new FakeTarget()],
      ["b", new FakeTarget()],
    ]);
    const calls: string[] = [];
    bindClicks({ a: () => calls.push("a"), b: () => calls.push("b") }, (id) => {
      const target = targets.get(id);
      if (!target) throw new Error(id);
      return target;
    });
    targets.get("b")?.fire("click", {});
    targets.get("a")?.fire("click", {});
    assert.deepEqual(calls, ["b", "a"]);
  });
});

describe("keyActions", () => {
  const press = (key: string) => {
    let prevented = false;
    return {
      event: { key, shiftKey: false, preventDefault: () => (prevented = true) },
      prevented: () => prevented,
    };
  };
  it("runs the mapped action and prevents the default", () => {
    const seen: string[] = [];
    const handler = keyActions({ Enter: (e) => seen.push(e.key) });
    const enter = press("Enter");
    handler(enter.event);
    assert.deepEqual(seen, ["Enter"]);
    assert.equal(enter.prevented(), true);
  });
  it("leaves other keys alone", () => {
    const handler = keyActions({ Enter: () => undefined });
    const tab = press("Tab");
    handler(tab.event);
    assert.equal(tab.prevented(), false);
  });
});

describe("runAndAnnounce", () => {
  it("announces success", async () => {
    const region = { textContent: "" as string | null };
    const ok = await runAndAnnounce(() => Promise.resolve(), region, "Removed.");
    assert.equal(ok, true);
    assert.equal(region.textContent, "Removed.");
  });
  it("announces the error instead", async () => {
    const region = { textContent: "" as string | null };
    const ok = await runAndAnnounce(() => Promise.reject(new Error("No router")), region, "x");
    assert.equal(ok, false);
    assert.equal(region.textContent, "No router");
  });
  it("turns any thrown value into text", () => {
    assert.equal(errorMessage("plain"), "plain");
  });
});
