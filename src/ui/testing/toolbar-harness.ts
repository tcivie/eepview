// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// The real toolbar page on a stand-in shell, and the moves of a user: click, key, type, drag.

import type { TabInfo } from "../contract.ts";
import { type EventInit2, type Stub, sleep, stubShell, until } from "./shell-stub.ts";

export const STATUS = {
  state: "ok",
  proxy: "127.0.0.1:4444",
  version: "2.0",
  detail: null,
  managed: true,
  paused: false,
};

export function tab(over: Partial<TabInfo> = {}): TabInfo {
  return {
    id: 1,
    url: "http://a.i2p/",
    title: "A",
    kind: "web",
    loading: false,
    canBack: true,
    canForward: true,
    active: true,
    zoom: 1,
    jsOn: true,
    bookmarked: false,
    ...over,
  };
}

export interface Toolbar {
  stub: Stub;
  /** An element of the page by id. */
  el(id: string): HTMLElement;
  /** Elements of the page by selector. */
  all(selector: string): HTMLElement[];
  /** A full click: pointer down, mouse down, pointer up, mouse up, click. */
  click(target: Element, init?: EventInit2): void;
  /** A key press on `target`. Returns true when the page cancelled it. */
  key(target: Element, key: string, init?: EventInit2): boolean;
  /** Types `text` into an input, as a user does. */
  type(input: HTMLInputElement, text: string): void;
  /** The input element with this id. */
  input(id: string): HTMLInputElement;
  /** Waits until a command was called `count` times, then lets stray repeats arrive. */
  calledOnce(cmd: string): Promise<boolean>;
  /** Replaces the tabs, as the shell does. */
  setTabs(tabs: TabInfo[]): void;
}

const isInput = (el: HTMLElement): el is HTMLInputElement => el.tagName === "INPUT";
const PRESS = ["pointerdown", "mousedown", "pointerup", "mouseup", "click"];

function clickOn(stub: Stub, target: Element, init: EventInit2 = {}): void {
  for (const type of PRESS) {
    const kind = type.startsWith("pointer") ? "PointerEvent" : "MouseEvent";
    target.dispatchEvent(stub.make(kind, type, { bubbles: true, ...init }));
  }
}

function keyOn(stub: Stub, target: Element, key: string, init: EventInit2 = {}): boolean {
  const event = stub.make("KeyboardEvent", "keydown", {
    key,
    bubbles: true,
    cancelable: true,
    ...init,
  });
  target.dispatchEvent(event);
  return event.defaultPrevented;
}

function typeInto(stub: Stub, input: HTMLInputElement, text: string): void {
  input.focus();
  input.value = text;
  input.dispatchEvent(stub.make("Event", "input", { bubbles: true }));
}

function buildHelpers(stub: Stub): Omit<Toolbar, "setTabs"> {
  const el = (id: string): HTMLElement => {
    const found = stub.doc.getElementById(id);
    if (!found) throw new Error(`no #${id} in the toolbar page`);
    return found;
  };
  return {
    stub,
    el,
    all: (selector) => [...stub.doc.querySelectorAll<HTMLElement>(selector)],
    click: (target, init) => clickOn(stub, target, init),
    key: (target, key, init) => keyOn(stub, target, key, init),
    type: (input, text) => typeInto(stub, input, text),
    input(id) {
      const found = el(id);
      if (!isInput(found)) throw new Error(`#${id} is not an input`);
      return found;
    },
    async calledOnce(cmd) {
      const reached = await until(() => stub.commands(cmd).length > 0);
      await sleep(150);
      return reached && stub.commands(cmd).length === 1;
    },
  };
}

/** Boots the toolbar with `tabs` open. Run once per test file. */
export async function bootToolbar(tabs: TabInfo[]): Promise<Toolbar> {
  const stub = stubShell("toolbar.html");
  let shown = 0;
  stub.answer("popup_open", () => ++shown);
  stub.answer("tab_list", () => tabs);
  stub.answer("router_status", () => STATUS);
  await import("../toolbar.ts");
  const helpers = buildHelpers(stub);
  await until(() => helpers.all('#tabs [role="tab"]').length === tabs.length);
  return { ...helpers, setTabs: (next) => stub.fire("tabs-changed", next) };
}
