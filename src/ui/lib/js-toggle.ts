// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

export function jsToggleLabel(on: boolean, host: string): string {
  const state = on ? "on" : "off";
  return host ? `JavaScript ${state} for ${host}` : `JavaScript ${state}`;
}
