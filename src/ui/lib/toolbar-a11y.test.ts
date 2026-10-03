import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const TOOLBAR_HTML = new URL("../toolbar.html", import.meta.url);
const BUTTON_RE = /<button\b[\s\S]*?<\/button>/g;

function attr(html: string, name: string): string {
  const match = html.match(new RegExp(`(?:^|\\s)${name}\\s*=\\s*"([^"]*)"`));
  return match?.[1] ?? "";
}

function accessibleNameOf(buttonHtml: string): string {
  const ariaLabel = attr(buttonHtml, "aria-label").trim();
  if (ariaLabel) return ariaLabel;
  const title = attr(buttonHtml, "title").trim();
  if (title) return title;
  return buttonHtml
    .replace(/<[^>]*>/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

describe("toolbar accessibility", () => {
  const html = readFileSync(TOOLBAR_HTML, "utf8");
  const buttons = html.match(BUTTON_RE) ?? [];

  it("finds the toolbar buttons", () => {
    assert.ok(buttons.length > 0, "toolbar.html should contain buttons");
  });

  for (const buttonHtml of buttons) {
    const id = attr(buttonHtml, "id") || "(no id)";
    it(`button #${id} has an accessible name`, () => {
      const name = accessibleNameOf(buttonHtml);
      assert.ok(
        name.length > 0,
        `toolbar button #${id} has no accessible name — add aria-label, title or text`,
      );
    });
  }
});
