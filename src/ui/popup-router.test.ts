// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// The router panel in the popup page, on a stand-in shell (docs/wiki/browser-ui.md, "Router
// panel"): what it shows and what its buttons send.

import assert from "node:assert/strict";
import { after, beforeEach, describe, it } from "node:test";
import { sleep, stubShell, until } from "./testing/shell-stub.ts";

const stub = stubShell("popup.html");
const { doc } = stub;
const history = { stepSeconds: 5, inBps: [100, 400, 250, 900], outBps: [50, 60, 70, 80] };
const FULL = {
  networkStatus: "OK",
  uptimeSeconds: 3700,
  routerKind: "i2pd",
  routerVersion: "2.50.1",
  javaVersion: null,
  bandwidthInBps: 1536,
  bandwidthOutBps: 2048,
  history,
  clientTunnels: 2,
  inboundTunnels: 3,
  outboundTunnels: 4,
  activePeers: 77,
  participatingTunnels: 5,
  buildSuccessRate: 0.5,
  knownRouters: 1,
  floodfills: 1,
};
const EMPTY = Object.fromEntries(Object.keys(FULL).map((k) => [k, null]));
let status = {
  state: "ok",
  proxy: "127.0.0.1:4444",
  version: "2.50.1",
  detail: null,
  managed: true,
  paused: false,
};
let stats: Record<string, unknown> = FULL;
let nextId = 500;

stub.answer("router_status", () => status);
stub.answer("router_stats", () => stats);
await import("./popup.ts");

const text = (id: string): string => doc.getElementById(id)?.textContent ?? "";
const attr = (id: string, name: string): string | null =>
  doc.getElementById(id)?.getAttribute(name) ?? null;
const button = (id: string): HTMLElement => {
  const found = doc.getElementById(id);
  if (!found) throw new Error(`no #${id}`);
  return found;
};
const click = (target: Element): void => {
  for (const type of ["mousedown", "mouseup", "click"]) {
    target.dispatchEvent(stub.make("MouseEvent", type, { bubbles: true }));
  }
};

async function showPanel(next = status, with_ = stats): Promise<number> {
  status = next;
  stats = with_;
  nextId += 1;
  stub.fire("popup-show", { id: nextId, kind: "router", anchorWidth: 28, data: null });
  await until(() => stub.commands("router_stats").length > 0);
  await sleep(60);
  return nextId;
}

after(() => stub.window.happyDOM.close());
beforeEach(() => {
  stub.fire("popup-closed", { id: nextId, kind: "router", refocus: false });
  stub.calls.length = 0;
});

describe("the router panel shows the router", () => {
  it("[browser-ui panel] shows the version, the peers and the tunnels", async () => {
    await showPanel(status, FULL);
    assert.ok(text("rp-version").includes("2.50.1"), text("rp-version"));
    assert.ok(text("rp-peers").includes("77"), text("rp-peers"));
    for (const n of ["3", "4", "5"]) assert.ok(text("rp-tunnels").includes(n), text("rp-tunnels"));
  });

  it("[browser-ui panel] shows the bandwidth and the proxy address", async () => {
    await showPanel(status, FULL);
    assert.ok(text("rp-bandwidth").includes("1.5 KB/s"), text("rp-bandwidth"));
    assert.ok(text("rp-bandwidth").includes("2.0 KB/s"), text("rp-bandwidth"));
    assert.ok(text("rp-proxy").includes("127.0.0.1:4444"));
  });

  it("[browser-ui panel] draws the sparkline of the last minutes", async () => {
    await showPanel(status, FULL);
    const line = attr("rp-spark-in", "d") ?? "";
    assert.ok(line.startsWith("M") && line.length > "M0 140".length, line);
    assert.notEqual(attr("rp-spark-out", "d"), "M0 140");
  });

  it('[browser-ui panel] shows "—" for a missing figure', async () => {
    await showPanel(status, EMPTY);
    for (const id of ["rp-uptime", "rp-peers", "rp-build"]) assert.equal(text(id), "—", id);
    assert.ok(text("rp-bandwidth").includes("—"), text("rp-bandwidth"));
    assert.doesNotMatch(text("rp-bandwidth"), /[0-9]/);
  });
});

describe("the router panel: more figures", () => {
  it("[browser-ui panel] shows the build success as a percentage with one decimal", async () => {
    await showPanel(status, FULL);
    assert.equal(text("rp-build"), "50.0%");
  });

  it("[browser-ui panel] shows the state with its own tone and title", async () => {
    await showPanel(status, FULL);
    const ok = { tone: attr("rp-head", "data-tone"), title: text("rp-title") };
    await showPanel({ ...status, state: "down" }, FULL);
    assert.notEqual(attr("rp-head", "data-tone"), ok.tone);
    assert.notEqual(text("rp-title"), ok.title);
  });
});

describe("the router panel buttons", () => {
  it('[browser-ui panel] offers "Pause I2P browsing" and sends connection_pause once', async () => {
    await showPanel({ ...status, state: "ok", paused: false }, FULL);
    assert.ok(text("rp-pause").includes("Pause I2P browsing"), text("rp-pause"));
    click(button("rp-pause"));
    assert.ok(await until(() => stub.commands("connection_pause").length > 0), "connection_pause");
    await sleep(100);
    assert.equal(stub.commands("connection_pause").length, 1);
  });

  it('[browser-ui panel] offers "Resume I2P browsing" while paused and sends connection_resume once', async () => {
    await showPanel({ ...status, paused: true }, FULL);
    assert.ok(text("rp-pause").includes("Resume I2P browsing"), text("rp-pause"));
    click(button("rp-pause"));
    assert.ok(
      await until(() => stub.commands("connection_resume").length > 0),
      "connection_resume",
    );
    await sleep(100);
    assert.equal(stub.commands("connection_resume").length, 1);
    assert.equal(stub.commands("connection_pause").length, 0);
  });
});

describe("the router panel: Restart, Stop and the Network page", () => {
  it("[browser-ui panel] Restart and Stop send router_control for a router eepview manages", async () => {
    await showPanel({ ...status, managed: true }, FULL);
    click(button("rp-restart"));
    click(button("rp-stop"));
    assert.ok(
      await until(() => stub.commands("router_control").length === 2),
      "two router_control calls",
    );
    assert.deepEqual(
      stub.commands("router_control").map((c) => c.args.action),
      ["restart", "stop"],
    );
  });
});

describe("the router panel: an unmanaged router and the Network page", () => {
  it("[browser-ui panel] Restart and Stop are off for another router, with the tooltip", async () => {
    await showPanel({ ...status, managed: false }, FULL);
    for (const id of ["rp-restart", "rp-stop"]) {
      const off =
        button(id).hasAttribute("disabled") || button(id).getAttribute("aria-disabled") === "true";
      assert.equal(off, true, `${id} is off`);
    }
    const tip = [button("rp-restart"), button("rp-stop"), doc.getElementById("rp-managed-note")];
    const shown = tip.some((el) =>
      (el?.getAttribute("title") ?? el?.textContent ?? "").includes(
        "Only for a router that eepview manages",
      ),
    );
    assert.ok(shown, "the tooltip reads: Only for a router that eepview manages");
    stub.calls.length = 0;
    click(button("rp-stop"));
    await sleep(100);
    assert.equal(stub.commands("router_control").length, 0, "a disabled button sent a command");
  });

  it("[browser-ui panel] Open Network page opens the Network page", async () => {
    await showPanel(status, FULL);
    click(button("rp-network"));
    assert.ok(
      await until(() => stub.calls.some((c) => c.cmd === "navigate" || c.cmd === "tab_new")),
      "opens a page",
    );
    const call = stub.calls.find((c) => c.cmd === "navigate" || c.cmd === "tab_new");
    assert.ok(JSON.stringify(call?.args).includes("eepview://stats"), JSON.stringify(call));
  });
});

describe("the panel refreshes", () => {
  it("[browser-ui panel] refreshes every 5 s while open and stops when it closes", async () => {
    const id = await showPanel(status, FULL);
    const before = stub.commands("router_stats").length;
    assert.ok(await until(() => stub.commands("router_stats").length > before, 7000), "a refresh");
    stub.fire("popup-closed", { id, kind: "router", refocus: false });
    await sleep(100);
    const after = stub.commands("router_stats").length;
    await sleep(6000);
    assert.equal(stub.commands("router_stats").length, after, "refreshed while closed");
  });
});
