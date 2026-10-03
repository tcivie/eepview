// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, sep } from "node:path";
import { describe, it } from "node:test";
import {
  CONSOLE_LINK_TEXT,
  consoleLink,
  NO_CONSOLE_TEXT,
  routerVersion,
  shouldRedetect,
} from "./console-links.ts";

interface Info {
  found: boolean;
  kind: "java" | "i2pd" | null;
  origin: string | null;
  version: string | null;
}

const NONE: Info = { found: false, kind: null, origin: null, version: null };
const JAVA: Info = {
  found: true,
  kind: "java",
  origin: "http://127.0.0.1:7657",
  version: "2.13.0",
};
const I2PD: Info = {
  found: true,
  kind: "i2pd",
  origin: "http://127.0.0.1:7070",
  version: null,
};

describe("R15 no console", () => {
  it("R15: the text is 'No router console found'", () => {
    assert.equal(NO_CONSOLE_TEXT, "No router console found");
  });

  it("R15: with no console the link is hidden and one line says so", () => {
    for (const info of [NONE, null, undefined]) {
      const view = consoleLink(info);
      assert.equal(view.found, false);
      assert.equal(view.label, null, "no link text when there is no console");
      assert.equal(view.note, "No router console found");
      assert.equal(view.title, null);
    }
  });
});

describe("R15 the one link of the detected router", () => {
  it("R15: the link text is 'I2P Router Console'", () => {
    assert.equal(CONSOLE_LINK_TEXT, "I2P Router Console");
  });

  it("R15: Java I2P shows the one link", () => {
    const view = consoleLink(JAVA);
    assert.equal(view.found, true);
    assert.equal(view.note, null);
    assert.equal(view.title, "Java I2P console");
    assert.equal(view.label, CONSOLE_LINK_TEXT);
  });

  it("R15: i2pd shows the same one link", () => {
    const view = consoleLink(I2PD);
    assert.equal(view.found, true);
    assert.equal(view.note, null);
    assert.equal(view.title, "i2pd web console");
    assert.equal(view.label, CONSOLE_LINK_TEXT);
  });

  it("R15: the view holds no per-page links", () => {
    for (const info of [JAVA, I2PD, NONE]) {
      const view: Record<string, unknown> = { ...consoleLink(info) };
      assert.deepEqual(Object.keys(view).sort(), ["found", "label", "note", "title"]);
    }
  });

  it("R15: the view does not change with the version of the router", () => {
    assert.deepEqual(consoleLink(JAVA), consoleLink({ ...JAVA, version: null }));
  });
});

describe("R18 router version, display only", () => {
  it("R18: the status version comes first", () => {
    assert.equal(routerVersion("2.10.0", JAVA), "2.10.0");
    assert.equal(routerVersion("2.10.0", I2PD), "2.10.0");
    assert.equal(routerVersion("2.10.0", null), "2.10.0");
    assert.equal(routerVersion("2.10.0", undefined), "2.10.0");
  });

  it("R18: with no status version, the console version shows", () => {
    assert.equal(routerVersion(null, JAVA), "2.13.0");
    assert.equal(routerVersion(null, { ...I2PD, version: "2.59.0" }), "2.59.0");
  });

  it("R18: with neither, the answer is null", () => {
    assert.equal(routerVersion(null, I2PD), null);
    assert.equal(routerVersion(null, NONE), null);
    assert.equal(routerVersion(null, null), null);
    assert.equal(routerVersion(null, undefined), null);
  });

  it("R18: the router panel and the home page show routerVersion", () => {
    const own = (f: string) => readFileSync(f, "utf8");
    const used = (pick: (f: string) => boolean) =>
      sources().some((f) => pick(f.slice(UI.length)) && /routerVersion/.test(own(f)));
    assert.ok(
      used((f) => /popup[\\/]router-panel/.test(f)),
      "the router panel uses routerVersion",
    );
    assert.ok(
      used((f) => /home|shared/.test(f)),
      "the home page uses routerVersion",
    );
  });
});

type State = "verifying" | "ok" | "building" | "down" | "not-i2p" | "outproxy";
const status = (state: State, paused = false) => ({
  state,
  proxy: "127.0.0.1:4444",
  version: null,
  detail: null,
  managed: false,
  paused,
});
const NOT_OK: State[] = ["verifying", "building", "down", "not-i2p", "outproxy"];

describe("R20 redetect when the router turns ok", () => {
  it("R20: an ok router that was not known triggers detection", () => {
    assert.equal(shouldRedetect(null, status("ok")), true);
  });

  it("R20: ok after any other state triggers detection", () => {
    for (const prev of NOT_OK) {
      assert.equal(shouldRedetect(status(prev), status("ok")), true, prev);
    }
  });
});

describe("R20 redetect when the router does not turn ok", () => {
  it("R20: ok after ok does not trigger again", () => {
    assert.equal(shouldRedetect(status("ok"), status("ok")), false);
  });

  it("R20: a state other than ok never triggers", () => {
    for (const next of NOT_OK) {
      assert.equal(shouldRedetect(null, status(next)), false, next);
      assert.equal(shouldRedetect(status("ok"), status(next)), false, next);
    }
  });

  it("R20: a paused connection never triggers", () => {
    assert.equal(shouldRedetect(null, status("ok", true)), false);
    assert.equal(shouldRedetect(status("down"), status("ok", true)), false);
  });

  it("R20: prev ok but paused still counts as ok", () => {
    assert.equal(shouldRedetect(status("ok", true), status("ok")), false);
  });

  it("R20: the UI uses shouldRedetect on router-status", () => {
    const used = sources().some((f) => /shouldRedetect/.test(readFileSync(f, "utf8")));
    assert.ok(used, "some page calls shouldRedetect");
  });
});

// ------------------------------------------------------------ source checks

const UI = join(import.meta.dirname, "..");

function walk(dir: string): string[] {
  const out: string[] = [];
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) {
      out.push(...walk(path));
    } else {
      out.push(path);
    }
  }
  return out;
}

const read = (name: string): string => readFileSync(join(UI, name), "utf8");
const sources = (): string[] =>
  walk(UI).filter((f) => /\.(ts|html)$/.test(f) && !/\.test\.ts$/.test(f));

describe("R15 no loopback link in any page", () => {
  it("R15: no page holds an http://127.0.0.1 link", () => {
    // Pages only: the test helpers under testing/ are not pages.
    const pages = sources().filter((f) => !f.includes(`${sep}testing${sep}`));
    for (const file of pages) {
      const text = readFileSync(file, "utf8");
      assert.ok(!/https?:\/\/(127\.0\.0\.1|localhost|\[::1\])/.test(text), file);
    }
  });

  it("R15: the router panel, the home page and Settings show the one console link", () => {
    for (const name of ["home.ts", "popup/router-panel.ts", "settings.ts"]) {
      assert.match(read(name), /console/i, name);
    }
    assert.ok(
      sources().some((f) => /consoleLink\(/.test(readFileSync(f, "utf8"))),
      "some page renders the link from consoleLink()",
    );
  });

  it("R15: no page keeps the per-page links of the first console", () => {
    for (const file of sources().filter((f) => !f.includes(`${sep}testing${sep}`))) {
      const text = readFileSync(file, "utf8");
      assert.ok(!/consoleLinks|CONSOLE_PAGE_(ORDER|LABELS)/.test(text), file);
    }
  });
});

describe("R12 the UI calls console_open with no argument", () => {
  it("R12: the UI calls console_open from the contract", () => {
    assert.match(read("contract.ts"), /console_open/);
    assert.ok(sources().some((f) => /console_open/.test(readFileSync(f, "utf8"))));
  });

  it("R12: the contract has no page argument and no ConsolePage type", () => {
    const contract = read("contract.ts");
    const line = contract.slice(contract.indexOf("console_open:"));
    assert.ok(!/^[^\n]*page/i.test(line), "console_open takes no page");
    assert.ok(!/ConsolePage/.test(contract), "no ConsolePage type");
  });

  it("R12: no call of console_open passes a page", () => {
    for (const file of sources().filter((f) => !f.includes(`${sep}testing${sep}`))) {
      const text = readFileSync(file, "utf8");
      assert.ok(!/console_open[^;\n]{0,60}page/i.test(text), file);
    }
  });

  it("R12: the open result has no no-page reason", () => {
    assert.ok(!/no-page/.test(read("contract.ts")));
    assert.match(read("contract.ts"), /no-console/);
  });

  it("R7: ConsoleInfo lists no pages", () => {
    const contract = read("contract.ts");
    const start = contract.indexOf("export type ConsoleInfo");
    const info = contract.slice(start, contract.indexOf("};", start));
    assert.ok(start >= 0 && !/pages/.test(info), info);
  });

  it("R23: TabInfo.kind includes console", () => {
    const contract = read("contract.ts");
    const start = contract.indexOf("export type TabInfo");
    const tab = contract.slice(start, contract.indexOf("};", start));
    assert.match(tab, /"console"/);
  });
});

describe("R6 the UI asks for detection", () => {
  it("R6: the contract has console_status, console_detect and console-changed", () => {
    const contract = read("contract.ts");
    assert.match(contract, /console_status/);
    assert.match(contract, /console_detect/);
    assert.match(contract, /console-changed/);
  });

  it("R6: the router panel, the home page and Settings call console_detect", () => {
    const text = ["popup/router-panel.ts", "home.ts", "settings.ts"].map(read);
    for (const source of text) {
      assert.match(source, /console_detect|detectConsole/);
    }
  });
});

describe("R16 no router configuration in eepview", () => {
  it("R16: Settings has no bandwidth, share, relay or subscription control", () => {
    const html = read("settings.html").toLowerCase();
    for (const word of [
      "bandwidth",
      "bw-in",
      "bw-out",
      "share",
      "transit",
      "relay",
      "subscription",
    ]) {
      assert.ok(!html.includes(word), `settings.html still has ${word}`);
    }
    const script = read("settings.ts");
    for (const word of ["share-out", "bw-in", "transit"]) {
      assert.ok(!script.includes(word), `settings.ts still has ${word}`);
    }
  });
});

describe("R16 the Router section", () => {
  it("R16: the Router section has the one console link", () => {
    const html = read("settings.html");
    const router = html.slice(html.indexOf('id="router"'), html.indexOf('id="about"'));
    assert.ok(router.length > 0, "Settings has a Router section");
    assert.match(router + read("settings.ts"), /console_open|console-link|data-console/);
    assert.match(read("settings.ts"), /console/i);
  });

  it("R16: the Router updates and Restore controls stay", () => {
    const html = read("settings.html");
    assert.match(html, /name="updates"/);
    assert.match(html, /id="restore-btn"/);
  });

  it("R16: the About list names the console probe and the console tab", () => {
    const html = read("settings.html");
    const about = html.slice(html.indexOf('id="about"'));
    const connects = about.slice(about.indexOf("Where eepview connects"));
    const list = connects.slice(0, connects.indexOf("</table>"));
    assert.match(list, /console/i, "the console probe");
    assert.match(list, /console tab/i, "the console tab");
    assert.ok(!/console (view|window)/i.test(list), "no separate console window or view");
  });

  it("R16: pause and resume of the connection stay", () => {
    const text = sources()
      .map((f) => readFileSync(f, "utf8"))
      .join("\n");
    assert.match(text, /connection_pause/);
    assert.match(text, /connection_resume/);
  });
});
