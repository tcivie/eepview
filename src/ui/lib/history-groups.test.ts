import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  dayKey,
  dayLabel,
  groupByDay,
  isBeforeCursor,
  newestFirst,
  oldestVisit,
  pageCursor,
  timeOfDay,
} from "./history-groups.ts";

const at = (y: number, m: number, d: number, h = 12, min = 0): number =>
  new Date(y, m - 1, d, h, min).getTime();
const NOW = at(2026, 10, 3, 15);

describe("dayLabel", () => {
  it("names today and yesterday", () => {
    assert.equal(dayLabel(at(2026, 10, 3, 0, 5), NOW), "Today");
    assert.equal(dayLabel(at(2026, 10, 2, 23, 59), NOW), "Yesterday");
  });
  it("uses the weekday and date for older days", () => {
    assert.equal(dayLabel(at(2026, 9, 28), NOW), "Monday 28 September");
  });
  it("adds the year when it differs", () => {
    assert.match(dayLabel(at(2025, 12, 31), NOW), /2025/);
  });
});

describe("groupByDay", () => {
  it("keeps the order and splits on local midnight", () => {
    const entries = [
      { id: "a", visited: at(2026, 10, 3, 9) },
      { id: "b", visited: at(2026, 10, 3, 1) },
      { id: "c", visited: at(2026, 10, 2, 22) },
      { id: "d", visited: at(2026, 9, 30, 8) },
    ];
    const groups = groupByDay(entries, NOW);
    assert.deepEqual(
      groups.map((g) => [g.label, g.entries.map((e) => e.id)]),
      [
        ["Today", ["a", "b"]],
        ["Yesterday", ["c"]],
        ["Wednesday 30 September", ["d"]],
      ],
    );
  });
  it("returns no groups for no entries", () => {
    assert.deepEqual(groupByDay([], NOW), []);
  });
});

describe("helpers", () => {
  it("builds a sortable day key", () => {
    assert.equal(dayKey(at(2026, 1, 5)), "2026-01-05");
  });
  it("formats the time of day", () => {
    assert.equal(timeOfDay(at(2026, 10, 3, 9, 7)), "09:07");
  });
  it("finds the oldest visit for the next page", () => {
    assert.equal(oldestVisit([{ visited: 5 }, { visited: 2 }, { visited: 9 }]), 2);
    assert.equal(oldestVisit([]), undefined);
  });
});

describe("paging", () => {
  const rows = [
    { id: "c", visited: 9 },
    { id: "b", visited: 4 },
    { id: "a", visited: 4 },
  ];
  it("uses the last row as the cursor", () => {
    assert.deepEqual(pageCursor(rows), { visited: 4, id: "a" });
    assert.equal(pageCursor([]), undefined);
  });
  it("keeps rows that tie on the timestamp", () => {
    const cursor = { visited: 4, id: "b" };
    assert.deepEqual(
      rows.filter((r) => isBeforeCursor(r, cursor)).map((r) => r.id),
      ["a"],
    );
    assert.equal(isBeforeCursor({ id: "z", visited: 1 }, undefined), true);
  });
  it("sorts newest first, then by id", () => {
    const shuffled = [rows[2], rows[0], rows[1]].filter((r) => r !== undefined);
    assert.deepEqual(
      shuffled.sort(newestFirst).map((r) => r.id),
      ["c", "b", "a"],
    );
  });
});
