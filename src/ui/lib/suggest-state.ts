export interface SuggestState<T> {
  items: T[];
  index: number;
  open: boolean;
}

export type SuggestKey = "ArrowDown" | "ArrowUp" | "Escape" | "Enter";

export function emptySuggest<T>(): SuggestState<T> {
  return { items: [], index: -1, open: false };
}

export function withItems<T>(items: T[]): SuggestState<T> {
  return { items, index: -1, open: items.length > 0 };
}

export function move<T>(state: SuggestState<T>, delta: number): SuggestState<T> {
  const count = state.items.length;
  if (count === 0) return state;
  const start = state.index < 0 && delta < 0 ? count : state.index;
  const index = (((start + delta) % count) + count) % count;
  return { ...state, index, open: true };
}

export function selected<T>(state: SuggestState<T>): T | undefined {
  return state.open ? state.items[state.index] : undefined;
}

export function isSuggestKey(key: string): key is SuggestKey {
  return key === "ArrowDown" || key === "ArrowUp" || key === "Escape" || key === "Enter";
}

export interface KeyOutcome<T> {
  state: SuggestState<T>;
  go?: T | "typed";
  revert?: boolean;
}

export function onKey<T>(state: SuggestState<T>, key: SuggestKey): KeyOutcome<T> {
  if (key === "ArrowDown") return { state: move(state, 1) };
  if (key === "ArrowUp") return { state: move(state, -1) };
  if (key === "Escape") {
    return state.open ? { state: { ...state, open: false, index: -1 } } : { state, revert: true };
  }
  return { state: emptySuggest<T>(), go: selected(state) ?? "typed" };
}
