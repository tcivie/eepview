// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { folderGroups, folderNames, NO_FOLDER_LABEL } from "./bookmark-groups.ts";

const items = [
  { id: "a", folder: "Reading" },
  { id: "b", folder: null },
  { id: "c", folder: "Forums" },
  { id: "d", folder: " Reading " },
  { id: "e", folder: "" },
];

describe("bookmark folders", () => {
  it("lists folder names once, sorted, without blanks", () => {
    assert.deepEqual(folderNames(items), ["Forums", "Reading"]);
  });
  it("groups by folder and puts loose bookmarks last", () => {
    const groups = folderGroups(items);
    assert.deepEqual(
      groups.map((g) => [g.label, g.items.map((i) => i.id)]),
      [
        ["Forums", ["c"]],
        ["Reading", ["a", "d"]],
        [NO_FOLDER_LABEL, ["b", "e"]],
      ],
    );
  });
  it("returns no groups for no bookmarks", () => {
    assert.deepEqual(folderGroups([]), []);
  });
});
