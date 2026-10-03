// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// The dev backend answers the report commands, so `?dev=1` shows the report page and the
// crash banner with sample data.

import assert from "node:assert/strict";
import { registerHooks } from "node:module";
import { describe, it } from "node:test";
import type { Backend } from "./ipc.ts";

registerHooks({
  load(url, context, nextLoad) {
    if (url.endsWith(".json")) {
      return nextLoad(url, { ...context, importAttributes: { type: "json" } });
    }
    return nextLoad(url, context);
  },
});

const backend = async (): Promise<Backend> => {
  Object.assign(globalThis, { window: { location: { search: "" } } });
  return (await import("./mock.ts")).mockBackend;
};

describe("dev backend: report commands", () => {
  it("previews the description, or (not given) when it is blank", async () => {
    const b = await backend();
    const args = { kind: "general", includeLog: true };
    const text = await b.invoke("report_preview", { ...args, description: "it broke" });
    assert.match(String(text), /it broke/);
    const blank = await b.invoke("report_preview", { ...args, description: "" });
    assert.match(String(blank), /\(not given\)/);
  });
  it("opens with a file name only, and answers the crash and delete commands", async () => {
    const b = await backend();
    const opened = (await b.invoke("report_open", {})) as { file: string; trimmed: boolean };
    assert.match(opened.file, /^eepview-report-.*\.txt$/);
    assert.equal(await b.invoke("diag_crash_status", {}), false);
    assert.equal(await b.invoke("diag_crash_dismiss", {}), undefined);
    assert.equal(await b.invoke("diag_logs_delete", {}), undefined);
  });
});
