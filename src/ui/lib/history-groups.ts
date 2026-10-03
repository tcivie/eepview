export interface Dated {
  visited: number;
}

export interface DayGroup<T extends Dated> {
  key: string;
  label: string;
  entries: T[];
}

const DAY_MS = 86_400_000;

function startOfDay(ts: number): number {
  const d = new Date(ts);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

export function dayKey(ts: number): string {
  const d = new Date(ts);
  const month = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${d.getFullYear()}-${month}-${day}`;
}

export function dayLabel(ts: number, now: number, locale = "en-GB"): string {
  const daysAgo = Math.round((startOfDay(now) - startOfDay(ts)) / DAY_MS);
  if (daysAgo === 0) return "Today";
  if (daysAgo === 1) return "Yesterday";
  const sameYear = new Date(ts).getFullYear() === new Date(now).getFullYear();
  const options: Intl.DateTimeFormatOptions = { weekday: "long", day: "numeric", month: "long" };
  if (!sameYear) options.year = "numeric";
  return new Intl.DateTimeFormat(locale, options).format(ts);
}

export function groupByDay<T extends Dated>(entries: T[], now: number): DayGroup<T>[] {
  const groups = new Map<string, DayGroup<T>>();
  for (const entry of entries) {
    const key = dayKey(entry.visited);
    const group = groups.get(key) ?? { key, label: dayLabel(entry.visited, now), entries: [] };
    group.entries.push(entry);
    groups.set(key, group);
  }
  return [...groups.values()];
}

export function timeOfDay(ts: number, locale = "en-GB"): string {
  return new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" }).format(ts);
}

export interface Cursor {
  visited: number;
  id: string;
}

export function pageCursor<T extends Dated & { id: string }>(entries: T[]): Cursor | undefined {
  const last = entries[entries.length - 1];
  return last ? { visited: last.visited, id: last.id } : undefined;
}

export function isBeforeCursor(entry: Cursor, cursor: Cursor | undefined): boolean {
  if (!cursor) return true;
  if (entry.visited !== cursor.visited) return entry.visited < cursor.visited;
  return entry.id < cursor.id;
}

export function newestFirst(a: Cursor, b: Cursor): number {
  if (a.visited !== b.visited) return b.visited - a.visited;
  return a.id < b.id ? 1 : -1;
}

export function oldestVisit<T extends Dated>(entries: T[]): number | undefined {
  return entries.length === 0 ? undefined : Math.min(...entries.map((e) => e.visited));
}
