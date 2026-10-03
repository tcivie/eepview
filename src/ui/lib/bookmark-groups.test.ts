// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { folderGroups, folderNames } from "./bookmark-groups.ts";

const mark = (url: string, folder: string | null) => ({ url, folder });

describe("[browser-ui 9] bookmark folders are one level deep", () => {
  const items = [
    mark("a.i2p", "Work"),
    mark("b.i2p", "Reading"),
    mark("c.i2p", "Work"),
    mark("d.i2p", null),
  ];
  it("[browser-ui 9] makes one group per folder, holding its bookmarks", () => {
    const work = folderGroups(items).find((g) => g.name === "Work");
    assert.deepEqual(
      work?.items.map((i) => i.url),
      ["a.i2p", "c.i2p"],
    );
    assert.deepEqual([...folderNames(items)].sort(), ["Reading", "Work"]);
  });
  it("[browser-ui 9] keeps every bookmark in exactly one group", () => {
    const urls = folderGroups(items).flatMap((g) => g.items.map((i) => i.url));
    assert.deepEqual([...urls].sort(), ["a.i2p", "b.i2p", "c.i2p", "d.i2p"]);
  });
  it("[browser-ui 9] a folder named with a slash is one folder, not a folder in a folder", () => {
    const names = folderNames([mark("a.i2p", "Work/Old")]);
    assert.deepEqual(names, ["Work/Old"]);
  });
});
