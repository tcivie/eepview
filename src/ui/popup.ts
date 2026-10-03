// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

import "./boot.ts";
import type { HintData, PopupKind, PopupShow, SuggestionsData } from "./contract.ts";
import { byId } from "./dom.ts";
import { call, devMode, on } from "./ipc.ts";
import { closeCurrent, currentPopup, forget, report, showCard } from "./popup/frame.ts";
import { openMenu, wireMenu } from "./popup/menu.ts";
import { openPanel, stopPanel, wireRouterPanel } from "./popup/router-panel.ts";
import { renderSuggestions, selectSuggestion, wireSuggestions } from "./popup/suggestions.ts";

const quiet = (): undefined => undefined;
const FOCUSED: readonly PopupKind[] = ["menu", "router"];

function renderHint(show: PopupShow): void {
  const data = show.data as HintData | null;
  byId("hint-title").textContent = data?.title ?? "";
  byId("hint-text").textContent = data?.text ?? "";
}

const RENDER: Record<PopupKind, (show: PopupShow) => void> = {
  suggestions: renderSuggestions,
  menu: openMenu,
  router: openPanel,
  hint: renderHint,
};

function stop(kind: PopupKind | null): void {
  if (kind === "router") stopPanel();
}

/** Renders the popup the toolbar opened, then reports its natural size to the shell. */
function show(next: PopupShow): void {
  const previous = currentPopup();
  if (previous && previous.kind !== next.kind) stop(previous.kind);
  showCard(next);
  RENDER[next.kind](next);
  report();
}

/** The menu and the router panel close on Esc, and when the focus leaves the popup. */
function wireClosing(): void {
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") closeCurrent(true);
  });
  window.addEventListener("blur", () => {
    const open = currentPopup();
    if (open && FOCUSED.includes(open.kind)) closeCurrent(false);
  });
}

async function previewData(kind: PopupKind): Promise<SuggestionsData | HintData | null> {
  if (kind === "suggestions") return { items: await call("suggest", { input: "i2p" }), index: 0 };
  if (kind === "hint")
    return { title: "Router ready", text: "Eepsites open through your I2P router." };
  return null;
}

/** `popup.html?dev=1&kind=router` previews one popup with sample data. */
function preview(): void {
  const raw = new URLSearchParams(window.location.search).get("kind");
  const kind = (Object.keys(RENDER) as PopupKind[]).find((k) => k === raw) ?? "router";
  previewData(kind)
    .then((data) => show({ id: 0, kind, anchorWidth: 560, data }))
    .catch(quiet);
}

wireMenu();
wireRouterPanel();
wireSuggestions();
wireClosing();
on("popup-show", show).catch(quiet);
on("popup-closed", ({ id }) => stop(forget(id))).catch(quiet);
on("popup-select", ({ index }) => selectSuggestion(index)).catch(quiet);
if (devMode) preview();
