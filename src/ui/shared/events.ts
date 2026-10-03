// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

export interface Listenable {
  addEventListener(type: string, listener: (event: Event) => void): void;
}

export interface ClosestTarget {
  closest(selector: string): unknown;
}

export interface TextRegion {
  textContent: string | null;
}

type Lookup = (id: string) => Listenable;

export function delegateClick<T>(
  root: Listenable,
  selector: string,
  handler: (match: T, event: MouseEvent) => void,
): void {
  root.addEventListener("click", (event: Event) => {
    const target = event.target as unknown as ClosestTarget | null;
    const match = target?.closest?.(selector) as T | null | undefined;
    if (match) handler(match, event as MouseEvent);
  });
}

export function bindClicks(map: Record<string, () => void>, lookup: Lookup): void {
  for (const [id, action] of Object.entries(map)) {
    lookup(id).addEventListener("click", () => action());
  }
}

export interface KeyLike {
  key: string;
  shiftKey: boolean;
  preventDefault(): void;
}

export function keyActions<E extends KeyLike>(
  map: Record<string, (event: E) => void>,
): (event: E) => void {
  return (event) => {
    const action = map[event.key];
    if (!action) return;
    event.preventDefault();
    action(event);
  };
}

export function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export async function runAndAnnounce(
  task: () => Promise<unknown>,
  region: TextRegion,
  success: string,
): Promise<boolean> {
  try {
    await task();
    region.textContent = success;
    return true;
  } catch (error) {
    region.textContent = errorMessage(error);
    return false;
  }
}
