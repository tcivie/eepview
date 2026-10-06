// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  formatPercent,
  formatRate,
  formatUptime,
  type StatsLike,
  statsText,
} from "./stats-view.ts";

describe("Network page numbers", () => {
  it("[browser-ui 11] shows a rate under 1 000 B/s in whole bytes", () => {
    assert.equal(formatRate(512), "512 B/s");
  });
  it("[browser-ui 11] shows a rate in kB/s with K = 1 000", () => {
    assert.equal(formatRate(1536), "1.54 kB/s");
  });
  it("[browser-ui 11] shows a rate in MB/s with K = 1 000", () => {
    assert.equal(formatRate(3_000_000), "3.00 MB/s");
  });
  it("[browser-ui 12] shows a ratio as a percentage with one decimal", () => {
    assert.equal(formatPercent(0.874), "87.4%");
    assert.equal(formatPercent(0.5), "50.0%");
    assert.equal(formatPercent(1), "100.0%");
  });
});

describe("[R54] formatRate", () => {
  it('[R54] null gives "—"', () => {
    assert.equal(formatRate(null), "—");
  });

  it("[R54] below 1 000 it gives whole bytes, `<n> B/s`", () => {
    assert.equal(formatRate(0), "0 B/s");
    assert.equal(formatRate(230), "230 B/s");
    assert.equal(formatRate(999), "999 B/s");
  });

  it("[R54] below 1 000 the byte count is rounded to an integer, with no decimal digit", () => {
    assert.equal(formatRate(230.4), "230 B/s");
    assert.equal(formatRate(229.6), "230 B/s");
    assert.doesNotMatch(formatRate(230), /\./);
  });
});

describe("[R54] formatRate units and digits", () => {
  it("[R54] from 1 000 up to 1 000 000 it gives `<v> kB/s` with v = bps / 1 000", () => {
    assert.equal(formatRate(1000), "1.00 kB/s");
    assert.equal(formatRate(1024), "1.02 kB/s");
    assert.equal(formatRate(53_910), "53.91 kB/s");
  });

  it("[R54] from 1 000 000 up it gives `<v> MB/s` with v = bps / 1 000 000", () => {
    assert.equal(formatRate(1_000_000), "1.00 MB/s");
    assert.equal(formatRate(2_500_000), "2.50 MB/s");
  });

  it("[R54] v has 2 decimals below 100, 1 decimal below 1 000, else none", () => {
    assert.equal(formatRate(99_990), "99.99 kB/s");
    assert.equal(formatRate(100_000), "100.0 kB/s");
    assert.equal(formatRate(123_500), "123.5 kB/s");
    assert.equal(formatRate(999_900), "999.9 kB/s");
    assert.equal(formatRate(99_990_000), "99.99 MB/s");
    assert.equal(formatRate(100_000_000), "100.0 MB/s");
    assert.equal(formatRate(1_500_000_000), "1500 MB/s");
  });

  it("[R54] the Java I2P console rate of 53.91 KBps shows as 53.91 kB/s, not 52.6", () => {
    assert.equal(formatRate(53_910), "53.91 kB/s");
    assert.doesNotMatch(formatRate(53_910), /52\.6/);
  });

  it('[R54] never writes "KB/s", the unit is "kB/s"', () => {
    for (const bps of [1000, 53_910, 123_500, 999_900]) {
      assert.doesNotMatch(formatRate(bps), /KB\/s/);
    }
  });
});

const STATS: StatsLike = {
  networkStatus: "OK",
  uptimeSeconds: 7380,
  routerKind: "i2pd",
  routerVersion: "2.50.0",
  javaVersion: null,
  bandwidthInBps: 1536,
  bandwidthOutBps: 512,
  history: null,
  clientTunnels: 6,
  participatingTunnels: 345,
  buildSuccessRate: 0.874,
  knownRouters: 2310,
  floodfills: 40,
};

describe("Network page", () => {
  const text = statsText(STATS);
  it("[browser-ui 11] shows the bandwidth rates with a unit, K = 1 000", () => {
    assert.equal(text.bandwidthIn, "1.54 kB/s");
    assert.equal(text.bandwidthOut, "512 B/s");
  });
  it("[browser-ui 12] shows the tunnel build success ratio as a percentage with one decimal", () => {
    assert.equal(text.buildRate, "87.4%");
  });
  it("Network page: shows the counts it was given", () => {
    assert.match(text.clientTunnels, /6/);
    assert.match(text.participatingTunnels, /345/);
    assert.match(text.knownRouters, /2310|2,310|2 310/);
  });
});

describe("[R42] uptime to its resolution", () => {
  it("[R42] with no resolution, a null one or one below 60 s, it keeps today's output", () => {
    for (const resolution of [undefined, null, 1, 30, 59]) {
      assert.equal(formatUptime(28_800, resolution), "8 h 0 min", `resolution ${resolution}`);
      assert.equal(formatUptime(7380, resolution), "2 h 3 min", `resolution ${resolution}`);
      assert.equal(formatUptime(93_784, resolution), "1 d 2 h", `resolution ${resolution}`);
      assert.equal(formatUptime(300, resolution), "5 min", `resolution ${resolution}`);
    }
    assert.equal(formatUptime(28_800), "8 h 0 min");
  });

  it('[R42] with a resolution of 3 600 s, 28 800 s is "8 h", not "8 h 0 min"', () => {
    assert.equal(formatUptime(28_800, 3600), "8 h");
  });

  it("[R42] with a resolution of 3 600 s, a day and hours drop the minutes", () => {
    assert.equal(formatUptime(7380, 3600), "2 h");
    assert.equal(formatUptime(93_784, 3600), "1 d 2 h");
  });

  it('[R42] with a resolution of 86 400 s, 172 800 s is "2 d"', () => {
    assert.equal(formatUptime(172_800, 86_400), "2 d");
    assert.equal(formatUptime(93_784, 86_400), "1 d");
  });
});

describe("[R42] uptime at a resolution of 60 s or finer than the unit", () => {
  it("[R42] with a resolution of 60 s, the minutes stay", () => {
    assert.equal(formatUptime(7380, 60), "2 h 3 min");
    assert.equal(formatUptime(28_800, 60), "8 h 0 min");
    assert.equal(formatUptime(300, 60), "5 min");
  });

  it("[R42] never shows a unit smaller than the resolution", () => {
    for (const [seconds, resolution, unit] of [
      [93_784, 3600, "min"],
      [93_784, 86_400, "h"],
      [172_800, 86_400, "min"],
      [28_800, 3600, "min"],
    ] as const) {
      const text = formatUptime(seconds, resolution);
      assert.equal(new RegExp(`\\b${unit}\\b`).test(text), false, `${text} shows ${unit}`);
    }
  });
});

describe("[R42] the Network page passes the resolution on", () => {
  it('[R42] statsText shows 28 800 s with a resolution of 3 600 s as "8 h"', () => {
    const text = statsText({ ...STATS, uptimeSeconds: 28_800, uptimeResolutionSeconds: 3600 });
    const shown = Object.values(text).filter((value) => /\b8 h\b/.test(value));
    assert.ok(shown.length > 0, `no field shows "8 h": ${JSON.stringify(text)}`);
    assert.ok(
      shown.every((value) => !value.includes("0 min")),
      `a field shows "0 min": ${JSON.stringify(shown)}`,
    );
  });

  it("[R42] statsText keeps today's uptime when the resolution is null", () => {
    const text = statsText({ ...STATS, uptimeSeconds: 28_800, uptimeResolutionSeconds: null });
    assert.ok(Object.values(text).some((value) => String(value).includes("8 h 0 min")));
  });
});
