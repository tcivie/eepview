import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { emptySuggest, isSuggestKey, move, onKey, selected, withItems } from "./suggest-state.ts";

const three = () => withItems(["a", "b", "c"]);

describe("suggestion state", () => {
  it("opens only when there are items", () => {
    assert.equal(withItems([]).open, false);
    assert.equal(three().open, true);
    assert.equal(three().index, -1);
  });
  it("moves down and wraps", () => {
    let state = three();
    state = move(state, 1);
    assert.equal(selected(state), "a");
    state = move(move(move(state, 1), 1), 1);
    assert.equal(selected(state), "a");
  });
  it("moves up from nothing to the last item", () => {
    assert.equal(selected(move(three(), -1)), "c");
  });
  it("does nothing with no items", () => {
    const empty = emptySuggest<string>();
    assert.equal(move(empty, 1), empty);
  });
  it("selects nothing while closed", () => {
    assert.equal(selected({ items: ["a"], index: 0, open: false }), undefined);
  });
});

describe("keyboard", () => {
  it("recognises the handled keys", () => {
    assert.equal(isSuggestKey("ArrowDown"), true);
    assert.equal(isSuggestKey("Tab"), false);
  });
  it("arrows move the highlight", () => {
    assert.equal(selected(onKey(three(), "ArrowDown").state), "a");
    assert.equal(selected(onKey(three(), "ArrowUp").state), "c");
  });
  it("Escape closes the list, then asks to revert", () => {
    const first = onKey(move(three(), 1), "Escape");
    assert.equal(first.state.open, false);
    assert.equal(first.revert, undefined);
    assert.equal(onKey(first.state, "Escape").revert, true);
  });
  it("Enter goes to the highlighted item or the typed text", () => {
    assert.equal(onKey(move(three(), 1), "Enter").go, "a");
    assert.equal(onKey(three(), "Enter").go, "typed");
    assert.equal(onKey(three(), "Enter").state.open, false);
  });
});
