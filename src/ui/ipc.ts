import { isTauri, invoke as tauriInvoke } from "@tauri-apps/api/core";
import { emit as tauriEmit, listen as tauriListen } from "@tauri-apps/api/event";
import type { CommandName, Commands, EventName, Events, UiEvents } from "./contract.ts";

export { errorMessage as errorText } from "./shared/events.ts";

export interface Backend {
  invoke(cmd: CommandName, args: object): Promise<unknown>;
  listen(event: EventName, handler: (payload: unknown) => void): Promise<() => void>;
  emit(event: keyof UiEvents, payload: unknown): Promise<void>;
}

export const devMode = new URLSearchParams(window.location.search).has("dev");
export const inShell = !devMode && isTauri();

const tauriBackend: Backend = {
  invoke: (cmd, args) => tauriInvoke(cmd, args as Record<string, unknown>),
  listen: (event, handler) => tauriListen(event, (e) => handler(e.payload)),
  emit: (event, payload) => tauriEmit(event, payload),
};

const NOT_IN_SHELL = "This page runs inside eepview. Add ?dev=1 to preview it with sample data.";

const offlineBackend: Backend = {
  invoke: () => Promise.reject(new Error(NOT_IN_SHELL)),
  listen: () => Promise.resolve(() => undefined),
  emit: () => Promise.resolve(),
};

function loadBackend(): Promise<Backend> {
  if (inShell) return Promise.resolve(tauriBackend);
  if (devMode) return import("./mock.ts").then((m) => m.mockBackend);
  return Promise.resolve(offlineBackend);
}

const backend = loadBackend();

export async function call<K extends CommandName>(
  cmd: K,
  args: Commands[K]["args"],
): Promise<Commands[K]["result"]> {
  const b = await backend;
  return (await b.invoke(cmd, args)) as Commands[K]["result"];
}

export async function on<K extends EventName>(
  event: K,
  handler: (payload: Events[K]) => void,
): Promise<() => void> {
  const b = await backend;
  return b.listen(event, (payload) => handler(payload as Events[K]));
}

export async function tell<K extends keyof UiEvents>(
  event: K,
  payload: UiEvents[K],
): Promise<void> {
  const b = await backend;
  await b.emit(event, payload);
}
