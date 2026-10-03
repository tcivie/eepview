export type ReportKind = "general" | "crash" | "blocked" | "router-down" | "load-failed";

const KINDS: readonly ReportKind[] = ["general", "crash", "blocked", "router-down", "load-failed"];

export const PLACEHOLDER = "What did you do, and what went wrong? Do not include site addresses.";

export const BROWSER_NOTE =
  "This opens GitHub in your normal web browser, outside I2P. GitHub sees your IP address and your GitHub account. Nothing is sent until you submit the issue on GitHub.";

export const DRAG_NOTE = "Drag this file into the GitHub issue to attach it.";

export const CRASH_TEXT = "eepview closed unexpectedly. Report the problem?";

function isKind(value: string): value is ReportKind {
  return (KINDS as readonly string[]).includes(value);
}

export function reportKind(search: string): ReportKind {
  const value = new URLSearchParams(search).get("kind") ?? "";
  return isKind(value) ? value : "general";
}

export function reportHref(kind: ReportKind): string {
  return `./report.html?kind=${kind}`;
}

export function openedMessage(result: { file: string; trimmed: boolean }): string {
  const saved = `Saved ${result.file} in your Downloads folder.`;
  const shorter = result.trimmed
    ? " The issue holds a shorter log than the file, so GitHub accepts its length."
    : "";
  return `${saved}${shorter} ${DRAG_NOTE}`;
}
