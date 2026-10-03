// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// Static requirement tests for the popup page (docs/wiki/browser-shell.md, "Toolbar popups"):
// the palette tokens and the transparent webview (rule 13), and the `popup` webview's
// permissions and its bundled page (rule 14).

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), "utf8");

interface Capability {
  identifier: string;
  webviews?: string[];
  permissions: string[];
}

const capability = (name: string): Capability =>
  JSON.parse(read(`../../src-tauri/capabilities/${name}.json`));

const POPUP_COMMANDS = [
  "popup_size",
  "popup_close",
  "navigate",
  "tab_new",
  "tab_list",
  "zoom_in",
  "zoom_out",
  "zoom_reset",
  "router_status",
  "router_stats",
  "connection_pause",
  "connection_resume",
  "router_control",
];

describe("rule 14: the popup webview's permissions", () => {
  it("[browser-shell popups 14] the popup capability names the popup webview only", () => {
    assert.deepEqual(capability("popup").webviews, ["popup"]);
  });

  it("[browser-shell popups 14] it grants exactly the commands the popups need", () => {
    const granted = capability("popup")
      .permissions.filter((p) => p.startsWith("allow-"))
      .sort();
    const wanted = POPUP_COMMANDS.map((c) => `allow-${c.replace(/_/g, "-")}`).sort();
    assert.deepEqual(granted, wanted);
  });

  it("[browser-shell popups 14] its other permissions only listen to events", () => {
    const others = capability("popup").permissions.filter((p) => !p.startsWith("allow-"));
    for (const permission of others) {
      assert.match(permission, /^core:event:allow-(listen|unlisten)$/, permission);
    }
  });
});

describe("rule 14: the toolbar and the page", () => {
  it("[browser-shell popups 14] popup_open stays with the toolbar", () => {
    assert.ok(!capability("popup").permissions.includes("allow-popup-open"));
    assert.ok(capability("toolbar-popup").permissions.includes("allow-popup-open"));
  });
});

describe("rule 14: tab webviews and the bundled page", () => {
  it("[browser-shell popups 14] no capability names a tab webview", () => {
    for (const name of ["default", "popup", "status", "toolbar-popup", "window"]) {
      for (const webview of capability(name).webviews ?? []) {
        assert.ok(!webview.startsWith("tab"), `${name} names ${webview}`);
        assert.ok(!webview.includes("*"), `${name} names the pattern ${webview}`);
      }
    }
  });
});

describe("rule 14: the bundled page", () => {
  it("[browser-shell popups 14] the popup page loads nothing from a remote origin", () => {
    const page = read("./popup.html");
    assert.ok(!page.includes("http://") && !page.includes("https://"), "a remote URL in the page");
    assert.ok(!page.includes('="//'), "a protocol-relative URL in the page");
  });
});

const POPUP_WORDS = ["popup", "hint-card", "router-panel", ".rp-", ".menu", "suggestion"];
const COLOR_FUNCTIONS = ["rgb", "rgba", "hsl", "hsla", "hwb", "lab", "lch", "oklab", "oklch"].map(
  (name) => `${name}(`,
);
const HEX_COLOR = /#[0-9a-f][0-9a-f][0-9a-f]/i;

function withoutComments(css: string): string {
  let out = css;
  for (let start = out.indexOf("/*"); start >= 0; start = out.indexOf("/*")) {
    const end = out.indexOf("*/", start);
    out = out.slice(0, start) + (end < 0 ? "" : out.slice(end + 2));
  }
  return out;
}

function rules(css: string): { selector: string; body: string }[] {
  return withoutComments(css)
    .split("}")
    .map((chunk) => chunk.split("{"))
    .filter((parts) => parts.length === 2)
    .map(([selector = "", body = ""]) => ({ selector: selector.trim(), body }));
}

const isPopupRule = (r: { selector: string }): boolean =>
  POPUP_WORDS.some((word) => r.selector.includes(word));

const hasRawColor = (r: { body: string }): boolean =>
  HEX_COLOR.test(r.body) || COLOR_FUNCTIONS.some((f) => r.body.includes(f));

describe("rule 13: palette tokens and a transparent webview", () => {
  const css = ["chrome.css", "ui.css"].flatMap((file) => rules(read(`./${file}`)));

  it("[browser-shell popups 13] the popup styles use no raw colour", () => {
    const raw = css.filter((r) => isPopupRule(r) && hasRawColor(r)).map((r) => r.selector);
    assert.deepEqual(raw, []);
  });

  it("[browser-shell popups 13] the popup page has a transparent background", () => {
    const page = css.filter((r) => /\.popup-(page|body)/.test(r.selector));
    const transparent = page.some((r) =>
      /background(?:-color)?\s*:\s*(transparent|none)\b/.test(r.body),
    );
    assert.ok(transparent, "no .popup-page or .popup-body rule sets a transparent background");
  });
});

describe("rule 13: the theme", () => {
  it("[browser-shell popups 13] the popup page follows the theme as the other pages do", () => {
    const page = read("./popup.html");
    assert.match(page, /theme-boot\.js/);
    assert.match(page, /theme\.css/);
  });
});
