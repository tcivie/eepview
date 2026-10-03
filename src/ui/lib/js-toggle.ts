export function jsToggleLabel(on: boolean, host: string): string {
  const state = on ? "on" : "off";
  return host ? `JavaScript ${state} for ${host}` : `JavaScript ${state}`;
}
