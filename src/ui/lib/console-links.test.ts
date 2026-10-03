// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, sep } from "node:path";
import { describe, it } from "node:test";
import {
  CONSOLE_PAGE_LABELS,
  CONSOLE_PAGE_ORDER,
  consoleLinks,
  NO_CONSOLE_TEXT,
  routerVersion,
  shouldRedetect,
} from "./console-links.ts";

type Page = "home" | "tunnels" | "addressbook" | "config" | "logs";
interface Info {
  found: boolean;
  kind: "java" | "i2pd" | null;
  origin: string | null;
  pages: Page[];
  version: string | null;
}

const NONE: Info = { found: false, kind: null, origin: null, pages: [], version: null };
const JAVA: Info = {
  found: true,
  kind: "java",
  origin: "http://127.0.0.1:7657",
  pages: ["home", "tunnels", "addressbook", "config", "logs"],
  version: "2.13.0",
};
const I2PD: Info = {
  found: true,
  kind: "i2pd",
  origin: "http://127.0.0.1:7070",
  pages: ["home", "tunnels", "config"],
  version: null,
};

const labels = (info: Info | null | undefined): string[] =>
  consoleLinks(info).links.map((l) => l.label);
const pages = (info: Info | null | undefined): string[] =>
  consoleLinks(info).links.map((l) => l.page);

describe("R7 page order and labels", () => {
  it("R7: the order is Console, Tunnels, Address book, Config, Logs", () => {
    assert.deepEqual([...CONSOLE_PAGE_ORDER], ["home", "tunnels", "addressbook", "config", "logs"]);
  });

  it("R7: the labels are the table labels", () => {
    assert.deepEqual(CONSOLE_PAGE_LABELS, {
      home: "Console",
      tunnels: "Tunnels",
      addressbook: "Address book",
      config: "Config",
      logs: "Logs",
    });
  });
});

describe("R15 no console", () => {
  it("R15: the text is 'No router console found'", () => {
    assert.equal(NO_CONSOLE_TEXT, "No router console found");
  });

  it("R15: with no console no link shows and one line says so", () => {
    for (const info of [NONE, null, undefined]) {
      const view = consoleLinks(info);
      assert.equal(view.found, false);
      assert.deepEqual(view.links, []);
      assert.equal(view.note, "No router console found");
      assert.equal(view.title, null);
    }
  });

  it("R15: found false hides the links even when pages are listed", () => {
    const view = consoleLinks({ ...NONE, pages: ["home", "config"] });
    assert.deepEqual(view.links, []);
    assert.equal(view.note, NO_CONSOLE_TEXT);
  });
});

describe("R15 links of the detected router", () => {
  it("R15: Java I2P shows all five links in order", () => {
    const view = consoleLinks(JAVA);
    assert.equal(view.found, true);
    assert.equal(view.note, null);
    assert.equal(view.title, "Java I2P console");
    assert.deepEqual(pages(JAVA), ["home", "tunnels", "addressbook", "config", "logs"]);
    assert.deepEqual(labels(JAVA), ["Console", "Tunnels", "Address book", "Config", "Logs"]);
  });

  it("R7: i2pd has no address book and no logs link", () => {
    const view = consoleLinks(I2PD);
    assert.equal(view.title, "i2pd web console");
    assert.equal(view.note, null);
    assert.deepEqual(labels(I2PD), ["Console", "Tunnels", "Config"]);
  });

  it("R15: the links follow the R7 order whatever the order of the pages", () => {
    const shuffled: Info = { ...JAVA, pages: ["logs", "config", "home", "addressbook", "tunnels"] };
    assert.deepEqual(pages(shuffled), ["home", "tunnels", "addressbook", "config", "logs"]);
  });

  it("R7: a page that the info does not list is not shown", () => {
    const some: Info = { ...JAVA, pages: ["config", "home"] };
    assert.deepEqual(labels(some), ["Console", "Config"]);
  });

  it("R15: each link carries its page key for console_open", () => {
    for (const link of consoleLinks(JAVA).links) {
      assert.equal(link.label, CONSOLE_PAGE_LABELS[link.page]);
    }
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

  it("R18: the version changes nothing in the links", () => {
    const without = consoleLinks({ ...JAVA, version: null });
    assert.deepEqual(consoleLinks(JAVA), without);
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

  it("R15: the router panel and the home page know the console links", () => {
    for (const name of ["home.ts", "popup/router-panel.ts"]) {
      assert.match(read(name), /console/i, name);
    }
  });

  it("R12: the UI calls console_open from the contract", () => {
    assert.match(read("contract.ts"), /console_open/);
    assert.ok(sources().some((f) => /console_open/.test(readFileSync(f, "utf8"))));
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
  it("R16: the Router section links to the console Config page", () => {
    const html = read("settings.html");
    const router = html.slice(html.indexOf('id="router"'), html.indexOf('id="about"'));
    assert.ok(router.length > 0, "Settings has a Router section");
    assert.match(router + read("settings.ts"), /console_open|console-link|data-console/);
    assert.match(read("settings.ts"), /config/);
  });

  it("R16: the Router updates and Restore controls stay", () => {
    const html = read("settings.html");
    assert.match(html, /name="updates"/);
    assert.match(html, /id="restore-btn"/);
  });

  it("R16: the About list names the console probe and the console view", () => {
    const html = read("settings.html");
    const about = html.slice(html.indexOf('id="about"'));
    const connects = about.slice(about.indexOf("Where eepview connects"));
    assert.match(connects.slice(0, connects.indexOf("</table>")), /console/i);
  });

  it("R16: pause and resume of the connection stay", () => {
    const text = sources()
      .map((f) => readFileSync(f, "utf8"))
      .join("\n");
    assert.match(text, /connection_pause/);
    assert.match(text, /connection_resume/);
  });
});

describe("R36 where eepview connects", () => {
  it("R36: the About row of the console names the statistics too", () => {
    const html = read("settings.html");
    const about = html.slice(html.indexOf('id="about"'));
    const connects = about.slice(about.indexOf("Where eepview connects"));
    const list = connects.slice(0, connects.indexOf("</table>"));
    assert.ok(
      list.includes("Router console check, router statistics and the console window"),
      "the row reads: Router console check, router statistics and the console window",
    );
  });
});
