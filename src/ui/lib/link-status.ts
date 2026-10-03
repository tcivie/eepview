export interface LinkHover {
  text: string;
  blocked: boolean;
}

export interface LinkStatusView {
  visible: boolean;
  prefix: string | null;
  text: string;
}

export const MAX_STATUS_CHARS = 512;
export const BLOCKED_PREFIX = "Blocked:";

export function cleanStatusText(text: string): string {
  const single = text.replace(/\s+/g, " ").trim();
  return single.length > MAX_STATUS_CHARS ? `${single.slice(0, MAX_STATUS_CHARS - 1)}…` : single;
}

export interface PillSize {
  width: number;
  height: number;
}

/** The size the shell gives the status webview: whole pixels, never cutting the pill. */
export function pillSize(rect: PillSize): PillSize {
  return { width: Math.ceil(rect.width), height: Math.ceil(rect.height) };
}

export function linkStatus(hover: LinkHover | null): LinkStatusView {
  const text = cleanStatusText(hover?.text ?? "");
  if (text === "") return { visible: false, prefix: null, text: "" };
  return { visible: true, prefix: hover?.blocked ? BLOCKED_PREFIX : null, text };
}
