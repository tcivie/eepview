import "./boot.ts";
import type { ClearRange, HistoryEntry } from "./contract.ts";
import { announce, byId } from "./dom.ts";
import { call, errorText, on } from "./ipc.ts";
import { displayUrl, hostOf } from "./lib/address.ts";
import { groupByDay, pageCursor, timeOfDay } from "./lib/history-groups.ts";
import { delegateClick, runAndAnnounce } from "./shared/events.ts";

const PAGE_SIZE = 50;
const SEARCH_DELAY_MS = 200;

const CLEAR_TEXT: Record<ClearRange, string> = {
  hour: "Pages you visited in the last hour are removed.",
  day: "Pages you visited in the last 24 hours are removed.",
  week: "Pages you visited in the last 7 days are removed.",
  all: "Your whole history is removed.",
};

let entries: HistoryEntry[] = [];
let done = false;
let loading = false;
let generation = 0;
let searchTimer = 0;

const status = (): HTMLElement => byId("history-status");
const query = (): string => byId<HTMLInputElement>("history-q").value.trim();

function clone(id: string): HTMLElement {
  const template = byId<HTMLTemplateElement>(id);
  return template.content.firstElementChild?.cloneNode(true) as HTMLElement;
}

function setText(root: ParentNode, selector: string, text: string): void {
  const el = root.querySelector(selector);
  if (el) el.textContent = text;
}

function entryRow(entry: HistoryEntry): HTMLElement {
  const row = clone("entry-template");
  const time = row.querySelector("time");
  time?.setAttribute("datetime", new Date(entry.visited).toISOString());
  setText(row, ".history-time", timeOfDay(entry.visited));
  setText(row, ".history-title", entry.title || displayUrl(entry.url));
  setText(row, ".history-host", hostOf(entry.url));
  row.querySelector("a")?.setAttribute("href", entry.url);
  const remove = row.querySelector<HTMLElement>("[data-remove]");
  remove?.setAttribute("data-remove", entry.id);
  remove?.setAttribute("aria-label", `Remove ${entry.title || hostOf(entry.url)} from history`);
  return row;
}

function render(): void {
  const groups = groupByDay(entries, Date.now()).map((group) => {
    const section = clone("day-template");
    setText(section, ".history-day-title", group.label);
    section.querySelector("ul")?.append(...group.entries.map(entryRow));
    return section;
  });
  byId("history").replaceChildren(...groups);
  const empty = byId("history-empty");
  empty.hidden = entries.length > 0 || loading;
  empty.textContent = query() ? `Nothing in your history matches “${query()}”.` : "No history yet.";
}

async function loadMore(): Promise<void> {
  if (loading || done) return;
  loading = true;
  const mine = generation;
  byId("history").setAttribute("aria-busy", "true");
  const before = pageCursor(entries);
  const page = await call("history_query", { query: { q: query(), before, limit: PAGE_SIZE } });
  if (mine !== generation) return;
  loading = false;
  byId("history").setAttribute("aria-busy", "false");
  entries = [...entries, ...page];
  done = page.length < PAGE_SIZE;
  render();
}

function showError(error: unknown): void {
  loading = false;
  byId("history-empty").hidden = false;
  byId("history-empty").textContent = errorText(error);
}

function reload(): void {
  generation += 1;
  entries = [];
  done = false;
  loading = false;
  loadMore().catch(showError);
}

function onSearch(): void {
  window.clearTimeout(searchTimer);
  searchTimer = window.setTimeout(reload, SEARCH_DELAY_MS);
}

function removeEntry(button: HTMLElement): void {
  const id = button.dataset.remove ?? "";
  const removed = entries.find((e) => e.id === id);
  const message = `Removed ${removed?.title || "the page"} from history.`;
  runAndAnnounce(() => call("history_remove", { id }), status(), message).then((ok) => {
    if (ok) entries = entries.filter((e) => e.id !== id);
    render();
  });
}

function chosenRange(): ClearRange {
  const checked = document.querySelector<HTMLInputElement>('input[name="range"]:checked');
  return (checked?.value ?? "hour") as ClearRange;
}

function setClearPanel(open: boolean): void {
  byId("clear-panel").hidden = !open;
  byId("clear-btn").setAttribute("aria-expanded", String(open));
  if (open) byId("clear-cancel").focus();
}

async function confirmClear(event: SubmitEvent): Promise<void> {
  event.preventDefault();
  const range = chosenRange();
  await call("history_clear", { range });
  setClearPanel(false);
  byId("clear-btn").focus();
  announce(status(), `Cleared. ${CLEAR_TEXT[range]}`);
  reload();
  if (new URLSearchParams(window.location.search).has("clear")) setClearPanel(true);
}

function wireClear(): void {
  byId("clear-btn").addEventListener("click", () =>
    setClearPanel(byId("clear-panel").hidden === true),
  );
  byId("clear-cancel").addEventListener("click", () => setClearPanel(false));
  byId("clear-panel").addEventListener("change", () => {
    byId("clear-warning").textContent = `${CLEAR_TEXT[chosenRange()]} This cannot be undone.`;
  });
  byId<HTMLFormElement>("clear-panel").addEventListener("submit", (e) => {
    confirmClear(e).catch((err) => announce(status(), errorText(err)));
  });
}

function watchScroll(): void {
  const observer = new IntersectionObserver((seen) => {
    if (seen.some((s) => s.isIntersecting)) loadMore().catch(showError);
  });
  observer.observe(byId("history-sentinel"));
}

function showHistoryState(): void {
  call("settings_get", {})
    .then((s) => {
      byId("history-off").hidden = s.history.enabled;
    })
    .catch(() => undefined);
}

byId<HTMLInputElement>("history-q").value =
  new URLSearchParams(window.location.search).get("q") ?? "";
byId("history-q").addEventListener("input", onSearch);
delegateClick(byId("history"), "[data-remove]", removeEntry);
wireClear();
watchScroll();
showHistoryState();
on("history-changed", reload).catch(() => undefined);
on("settings-changed", (s) => {
  byId("history-off").hidden = s.history.enabled;
}).catch(() => undefined);
reload();
if (new URLSearchParams(window.location.search).has("clear")) setClearPanel(true);
