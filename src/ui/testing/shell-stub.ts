// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// A stand-in for the Tauri shell, for tests that run a bundled page in a DOM (happy-dom).
// It answers the page's commands, records every call and every event the page sends, and
// delivers Rust events to the page's listeners. It knows nothing of the pages' code.

import { readFileSync } from "node:fs";
import { Window } from "happy-dom";

export interface Call {
  cmd: string;
  args: Record<string, unknown>;
}

export type Answer = (args: Record<string, unknown>) => unknown;

export type EventKind = "Event" | "MouseEvent" | "PointerEvent" | "KeyboardEvent";

export interface Stub {
  window: Window;
  /** The page's document, typed as the DOM library types it. */
  doc: Document;
  /** Builds an event of the page's window. */
  make(kind: EventKind, type: string, init?: EventInit & { key?: string }): Event;
  /** Dispatches an event on the page's window (a focus loss, for example). */
  dispatchOnWindow(event: Event): void;
  /** Every command the page called, in order (the event plugin's own calls left out). */
  calls: Call[];
  /** Every event the page sent with `emit`, in order. */
  sent: { event: string; payload: unknown }[];
  /** Calls to one command. */
  commands(cmd: string): Call[];
  /** Delivers a Rust event to the page's listeners. */
  fire(event: string, payload: unknown): void;
  /** Sets the answer to a command. */
  answer(cmd: string, answer: Answer): void;
}

const GLOBALS = [
  "window",
  "document",
  "Node",
  "Element",
  "HTMLElement",
  "HTMLInputElement",
  "HTMLButtonElement",
  "HTMLFormElement",
  "HTMLTemplateElement",
  "HTMLAnchorElement",
  "SVGElement",
  "Event",
  "KeyboardEvent",
  "MouseEvent",
  "PointerEvent",
  "FocusEvent",
  "InputEvent",
  "CustomEvent",
  "MutationObserver",
  "getComputedStyle",
  "matchMedia",
  "localStorage",
  "requestAnimationFrame",
  "cancelAnimationFrame",
];

export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Waits until `done()` holds, up to `ms`. Returns whether it held. */
export async function until(done: () => boolean, ms = 2000): Promise<boolean> {
  const end = Date.now() + ms;
  while (!done() && Date.now() < end) await sleep(10);
  return done();
}

function bodyOf(file: string): string {
  const html = readFileSync(new URL(`../${file}`, import.meta.url), "utf8");
  return /<body[^>]*>([\s\S]*)<\/body>/.exec(html)?.[1] ?? "";
}

function installGlobals(win: Window): void {
  const values: Record<string, unknown> = { isTauri: true };
  for (const name of GLOBALS) values[name] = Reflect.get(win, name);
  Object.assign(globalThis, values, { window: win });
}

type Callback = (event: object) => void;

/** The wire between a page and the stand-in shell. */
class Wire {
  readonly calls: Call[] = [];
  readonly sent: Stub["sent"] = [];
  readonly answers = new Map<string, Answer>();
  private readonly callbacks = new Map<number, Callback>();
  private readonly listeners = new Map<string, number[]>();
  private next = 0;

  invoke = async (cmd: string, args: Record<string, unknown> = {}): Promise<unknown> => {
    if (cmd === "plugin:event|listen") return this.listen(String(args.event), Number(args.handler));
    if (cmd === "plugin:event|emit") {
      this.sent.push({ event: String(args.event), payload: args.payload });
      return null;
    }
    if (cmd.startsWith("plugin:")) return null;
    this.calls.push({ cmd, args });
    return this.answers.get(cmd)?.(args) ?? null;
  };

  transformCallback = (callback: Callback): number => {
    this.callbacks.set(++this.next, callback);
    return this.next;
  };

  private listen(event: string, handler: number): number {
    this.listeners.set(event, [...(this.listeners.get(event) ?? []), handler]);
    return ++this.next;
  }

  fire(event: string, payload: unknown): void {
    for (const id of this.listeners.get(event) ?? [])
      this.callbacks.get(id)?.({ event, id, payload });
  }
}

/** Builds the DOM of `file` (a page in `src/ui`) and the stub. Import the page's script after. */
export function stubShell(file: string): Stub {
  const win = new Window({ url: "http://localhost/" });
  win.document.body.innerHTML = bodyOf(file);
  const wire = new Wire();
  Object.assign(win, {
    __TAURI_INTERNALS__: {
      invoke: wire.invoke,
      transformCallback: wire.transformCallback,
      unregisterCallback: () => undefined,
    },
    __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener: () => undefined },
  });
  installGlobals(win);
  const Ctors: Record<EventKind, new (type: string, init?: object) => unknown> = {
    Event: win.Event,
    MouseEvent: win.MouseEvent,
    PointerEvent: win.PointerEvent,
    KeyboardEvent: win.KeyboardEvent,
  };
  return {
    window: win,
    doc: win.document as unknown as Document,
    make: (kind, type, init) => new Ctors[kind](type, init) as Event,
    dispatchOnWindow: (event) => win.dispatchEvent(event as never),
    calls: wire.calls,
    sent: wire.sent,
    commands: (cmd) => wire.calls.filter((c) => c.cmd === cmd),
    fire: (event, payload) => wire.fire(event, payload),
    answer: (cmd, answer) => wire.answers.set(cmd, answer),
  };
}
