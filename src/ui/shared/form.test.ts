import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { checkMatching, setFieldError } from "./form.ts";

describe("checkMatching", () => {
  it("checks only the input with the value", () => {
    const inputs = [
      { value: "system", checked: true },
      { value: "dark", checked: false },
    ];
    checkMatching(inputs, "dark");
    assert.deepEqual(
      inputs.map((i) => i.checked),
      [false, true],
    );
  });
});

describe("setFieldError", () => {
  const fakes = () => {
    const attributes = new Map<string, string>();
    const classes = new Set<string>();
    const field = { setAttribute: (n: string, v: string) => attributes.set(n, v) };
    const hint = {
      textContent: "" as string | null,
      classList: {
        toggle: (token: string, force?: boolean) =>
          force ? classes.add(token) : classes.delete(token),
      },
    };
    return { attributes, classes, field, hint };
  };
  it("shows the error and marks the field invalid", () => {
    const f = fakes();
    setFieldError(f.field, f.hint, "Not I2P.", "Normal hint.");
    assert.equal(f.attributes.get("aria-invalid"), "true");
    assert.equal(f.hint.textContent, "Not I2P.");
    assert.equal(f.classes.has("field-error"), true);
  });
  it("restores the normal hint", () => {
    const f = fakes();
    setFieldError(f.field, f.hint, "Not I2P.", "Normal hint.");
    setFieldError(f.field, f.hint, null, "Normal hint.");
    assert.equal(f.attributes.get("aria-invalid"), "false");
    assert.equal(f.hint.textContent, "Normal hint.");
    assert.equal(f.classes.has("field-error"), false);
  });
});
