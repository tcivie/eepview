// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// Tests for the layer rule of the style check (docs/wiki/ui-components.md, "The style check"):
// every style rule sits inside a layer that theme.css declares, so `[hidden]` in @layer state
// always wins. The crash banner's Dismiss broke once because its rules sat outside a layer.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

const SCRIPT = fileURLToPath(new URL("../../scripts/css-layers.awk", import.meta.url));
const LAYERS = "base components pages state";

interface Result {
  ok: boolean;
  out: string;
}

/** A UTF-8 locale that exists on the runner: macOS has en_US.UTF-8, Linux has C.UTF-8. */
const UTF8 = process.platform === "darwin" ? "en_US.UTF-8" : "C.UTF-8";

/** Runs the check. Exit 1 is a found fault; any other failure is an awk crash and throws. */
function check(css: string, locale = "C"): Result {
  const dir = mkdtempSync(join(tmpdir(), "css-layers-"));
  const file = join(dir, "sample.css");
  writeFileSync(file, css);
  try {
    const out = execFileSync("awk", ["-v", `layers=${LAYERS}`, "-f", SCRIPT, file], {
      encoding: "utf8",
      env: { ...process.env, LC_ALL: locale },
    });
    return { ok: true, out };
  } catch (error) {
    const failed = error as { status?: number; stdout?: string; stderr?: string };
    if (failed.status !== 1) {
      throw new Error(`awk crashed (exit ${failed.status}): ${failed.stderr ?? ""}`);
    }
    return { ok: false, out: String(failed.stdout ?? "") };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

const read = (path: string): string => readFileSync(new URL(path, import.meta.url), "utf8");

describe("the layer rule of the style check passes", () => {
  it("rules inside declared layers", () => {
    assert.equal(check("@layer pages {\n  .a { display: flex; }\n}\n").ok, true);
    assert.equal(
      check("@layer state {\n  @media (width < 40rem) { .a { gap: 0; } }\n}\n").ok,
      true,
    );
  });

  it("the theme forms: the order statement, :root blocks and wrapped :root blocks", () => {
    const theme = [
      "@layer base, components, pages, state;",
      ":root { --x: 1; }",
      '@media (prefers-color-scheme: dark) {\n  :root:not([data-theme="light"]) { --x: 2; }\n}',
    ].join("\n");
    assert.equal(check(theme).ok, true);
  });
});

describe("the layer rule of the style check fails", () => {
  it("a rule outside every layer, and names its line", () => {
    const result = check(
      "@layer pages {\n  .a { gap: 0; }\n}\n\n.crash-banner {\n  display: flex;\n}\n",
    );
    assert.equal(result.ok, false);
    assert.match(result.out, /:5: rule outside @layer: \.crash-banner/);
  });

  it("an indented rule outside every layer", () => {
    assert.equal(check("  .a { display: flex; }\n").ok, false);
  });

  it("a top-level @media or @supports that holds a rule other than :root", () => {
    assert.equal(
      check("@media (width < 40rem) {\n  .crash-banner { display: flex; }\n}\n").ok,
      false,
    );
    assert.equal(check("@supports (display: grid) {\n  .a { display: grid; }\n}\n").ok, false);
  });

  it("a layer name that theme.css does not declare", () => {
    const result = check("@layer page {\n  .a { display: flex; }\n}\n");
    assert.equal(result.ok, false);
    assert.match(result.out, /@layer page is not in the order/);
  });
});

describe("the layer rule of the style check", () => {
  it("reads multibyte text in a UTF-8 locale, and still finds a fault after it", () => {
    const css = '@layer pages {\n  .a::before { content: "✓ ✕ –"; }\n}\n.b { gap: 0; }\n';
    const result = check(css, UTF8);
    assert.equal(result.ok, false);
    assert.match(result.out, /:4: rule outside @layer: \.b/);
  });

  it("ignores braces in comments", () => {
    assert.equal(
      check("/* .a { } */\n@layer pages {\n  /* } .b { */\n  .c { gap: 0; }\n}\n").ok,
      true,
    );
  });

  it("passes every stylesheet of the app, and the crash banner sits in @layer pages", () => {
    for (const name of ["ui.css", "pages.css", "chrome.css", "theme.css"]) {
      assert.equal(check(read(`./${name}`)).ok, true, name);
    }
    const pages = read("./pages.css");
    const layer = pages.indexOf("@layer pages {");
    assert.ok(layer >= 0 && pages.indexOf(".crash-banner {") > layer);
  });
});
