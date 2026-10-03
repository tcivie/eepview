// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { onKey, selected, withItems } from "./suggest-state.ts";

const ITEMS = ["notbob.i2p", "reg.i2p", "stats.i2p"];

describe("address suggestions keyboard", () => {
  it("[browser-ui 4] Down moves the highlight to the next entry, Up moves it back", () => {
    const first = onKey(withItems(ITEMS), "ArrowDown").state;
    const second = onKey(first, "ArrowDown").state;
    assert.notEqual(selected(second), selected(first));
    assert.equal(selected(first), ITEMS[0]);
    assert.equal(selected(second), ITEMS[1]);
    assert.equal(selected(onKey(second, "ArrowUp").state), ITEMS[0]);
  });
  it("[browser-ui 5] Enter opens the highlighted entry", () => {
    const state = onKey(onKey(withItems(ITEMS), "ArrowDown").state, "ArrowDown").state;
    assert.equal(onKey(state, "Enter").go, ITEMS[1]);
  });
  it("[browser-ui 6] Esc closes the list", () => {
    const open = onKey(withItems(ITEMS), "ArrowDown").state;
    assert.equal(open.open, true);
    assert.equal(onKey(open, "Escape").state.open, false);
  });
});
