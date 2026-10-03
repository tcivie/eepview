// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { groupByDay, isBeforeCursor, newestFirst, pageCursor } from "./history-groups.ts";

const at = (month: number, day: number, hour: number): number =>
  new Date(2026, month, day, hour, 0).getTime();
const NOW = at(9, 3, 15);
const entry = (visited: number, id: string) => ({ id, visited });

describe("[browser-ui 8] History groups by day", () => {
  const groups = groupByDay(
    [
      entry(at(9, 3, 9), "a"),
      entry(at(9, 3, 1), "b"),
      entry(at(9, 2, 22), "c"),
      entry(at(8, 28, 11), "d"),
      entry(at(8, 28, 9), "e"),
      entry(at(8, 20, 9), "f"),
    ],
    NOW,
  );
  it('[browser-ui 8] shows "Today", then "Yesterday", then one group per older date', () => {
    assert.equal(groups.length, 4);
    assert.equal(groups[0]?.label, "Today");
    assert.equal(groups[1]?.label, "Yesterday");
    assert.notEqual(groups[2]?.label, groups[3]?.label);
  });
  it("[browser-ui 8] puts the entries of one day into one group", () => {
    assert.deepEqual(
      groups.map((g) => g.entries.map((e) => e.id).join("")),
      ["ab", "c", "de", "f"],
    );
  });
  it("[browser-ui 8] names each older group by its date", () => {
    assert.match(groups[2]?.label ?? "", /28/);
    assert.match(groups[3]?.label ?? "", /20/);
  });
});

describe("[ipc-contract history-1] order and cursor", () => {
  it("[ipc-contract history-1] sorts newest first by visited desc, then id desc", () => {
    const sorted = [entry(100, "1"), entry(200, "1"), entry(100, "2")].sort(newestFirst);
    assert.deepEqual(
      sorted.map((e) => `${e.visited}/${e.id}`),
      ["200/1", "100/2", "100/1"],
    );
  });
  it("[ipc-contract history-1] the cursor of a page is the {visited, id} of its last entry", () => {
    const page = [entry(300, "9"), entry(200, "4")];
    assert.deepEqual(pageCursor(page), { visited: 200, id: "4" });
    assert.equal(pageCursor([]), undefined);
  });
  it("[ipc-contract history-1] an entry is before the cursor when it comes later in the order", () => {
    const cursor = { visited: 100, id: "5" };
    assert.equal(isBeforeCursor({ visited: 50, id: "9" }, cursor), true);
    assert.equal(isBeforeCursor({ visited: 100, id: "4" }, cursor), true);
    assert.equal(isBeforeCursor({ visited: 100, id: "6" }, cursor), false);
    assert.equal(isBeforeCursor({ visited: 200, id: "1" }, cursor), false);
    assert.equal(isBeforeCursor(cursor, cursor), false);
  });
});
