// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

export interface FindCount {
  query: string;
  matches: number | null;
  active: number | null;
}

export type FindTone = "idle" | "found" | "missing";

export function findTone(result: FindCount | null): FindTone {
  if (!result || result.query === "") return "idle";
  if (result.matches === null) return "found";
  return result.matches > 0 ? "found" : "missing";
}

export function findCountText(result: FindCount | null): string {
  const tone = findTone(result);
  if (tone === "idle" || !result) return "";
  if (tone === "missing") return "No matches";
  if (result.matches === null) return "—";
  return `${result.active ?? 1} of ${result.matches}`;
}
