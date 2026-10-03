// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import { siteMark } from "./lib/site-icon.ts";

/** Shows the site icon in `el`, or the letter chip when there is none. */
export function renderSiteMark(el: Element, icon: string | null | undefined, letter: string): void {
  const mark = siteMark(icon, letter);
  el.classList.toggle("has-icon", mark.kind === "icon");
  if (mark.kind === "letter") {
    el.textContent = mark.text;
    return;
  }
  const img = document.createElement("img");
  img.src = mark.src;
  img.alt = "";
  img.decoding = "async";
  img.draggable = false;
  el.replaceChildren(img);
}
