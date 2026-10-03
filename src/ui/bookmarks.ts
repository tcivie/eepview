import "./boot.ts";
import { announce, byId } from "./dom.ts";

interface Bookmark {
  id: string;
  name: string;
  address: string;
  folder: string;
}

const UNFILED = "";
const FOLDERS = ["Forums", "Reference", "Friends", UNFILED];
const ADDRESS_HINT = "A .i2p name or a .b32.i2p address.";
const EEPSITE_SCHEME = "http:";
const I2P_HOST = /^(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+i2p$/;

let nextId = 1;
const sample = (name: string, address: string, folder: string): Bookmark => ({
  id: `bm-${nextId++}`,
  name,
  address,
  folder,
});

let bookmarks: Bookmark[] = [
  sample("I2P Forum", "i2pforum.i2p", "Forums"),
  sample("zzz.i2p development forum", "zzz.i2p", "Forums"),
  sample("Ramble", "ramble.i2p", "Forums"),
  sample("Site uptime list", "notbob.i2p", "Reference"),
  sample("Network statistics", "stats.i2p", "Reference"),
  sample("Planet I2P", "planet.i2p", "Reference"),
  sample("Book club", "ukeu3k5oycgaauneqgtnvselmt4yemvoilkln7jpvamvfx7dnkdq.b32.i2p", "Friends"),
  sample("Legwork search", "legwork.i2p", UNFILED),
];

let editingId: string | null = null;

const folderLabel = (folder: string): string => (folder === UNFILED ? "Not in a folder" : folder);
const countLabel = (n: number): string => (n === 1 ? "1 site" : `${n} sites`);
const status = (): HTMLElement => byId("bookmark-status");

function hostOf(address: string): string {
  const trimmed = address.trim().toLowerCase();
  const withoutScheme = trimmed.replace(/^https?:/, "").replace(/^\/+/, "");
  return withoutScheme.split(/[/?#]/)[0] ?? "";
}

export function isI2pAddress(address: string): boolean {
  return I2P_HOST.test(hostOf(address));
}

function cloneTemplate(id: string): HTMLElement {
  const template = byId<HTMLTemplateElement>(id);
  return template.content.firstElementChild?.cloneNode(true) as HTMLElement;
}

function setText(root: ParentNode, selector: string, text: string): void {
  const el = root.querySelector(selector);
  if (el) el.textContent = text;
}

function fillRow(row: HTMLElement, bookmark: Bookmark): void {
  const link = row.querySelector<HTMLAnchorElement>(".row-link");
  link?.setAttribute("href", new URL(`${EEPSITE_SCHEME}${hostOf(bookmark.address)}`).href);
  setText(row, ".row-name", bookmark.name);
  setText(row, ".row-addr", hostOf(bookmark.address));
  for (const button of row.querySelectorAll<HTMLButtonElement>("button[data-action]")) {
    const verb = button.dataset.action === "edit" ? "Edit" : "Delete";
    button.dataset.id = bookmark.id;
    button.setAttribute("aria-label", `${verb} ${bookmark.name}`);
    button.title = verb;
  }
}

function renderRow(bookmark: Bookmark): HTMLElement {
  const row = cloneTemplate("row-template");
  fillRow(row, bookmark);
  return row;
}

function renderFolder(folder: string, items: Bookmark[]): HTMLElement {
  const section = cloneTemplate("folder-template");
  const headingId = `folder-${folder || "unfiled"}`.toLowerCase();
  section.setAttribute("aria-labelledby", headingId);
  const name = section.querySelector(".folder-name");
  name?.setAttribute("id", headingId);
  setText(section, ".folder-name", folderLabel(folder));
  setText(section, ".folder-count", countLabel(items.length));
  section.querySelector(".rows")?.append(...items.map(renderRow));
  return section;
}

function render(): void {
  const groups = FOLDERS.map((folder) => ({
    folder,
    items: bookmarks.filter((b) => b.folder === folder),
  })).filter((group) => group.items.length > 0);
  byId("folders").replaceChildren(...groups.map((g) => renderFolder(g.folder, g.items)));
}

function fillFolderSelect(selected: string): void {
  const select = byId<HTMLSelectElement>("bm-folder");
  select.replaceChildren(
    ...FOLDERS.map((folder) => new Option(folderLabel(folder), folder, false, folder === selected)),
  );
}

function setAddressError(message: string | null): void {
  const input = byId<HTMLInputElement>("bm-address");
  const hint = byId("bm-address-hint");
  input.setAttribute("aria-invalid", message ? "true" : "false");
  hint.textContent = message ?? ADDRESS_HINT;
  hint.classList.toggle("field-error", message !== null);
}

function openEditor(bookmark: Bookmark | null): void {
  editingId = bookmark?.id ?? null;
  byId("editor-title").textContent = bookmark ? "Edit bookmark" : "Add bookmark";
  byId<HTMLInputElement>("bm-name").value = bookmark?.name ?? "";
  byId<HTMLInputElement>("bm-address").value = bookmark?.address ?? "";
  fillFolderSelect(bookmark?.folder ?? UNFILED);
  setAddressError(null);
  byId<HTMLDialogElement>("editor").showModal();
}

function readEditor(): Omit<Bookmark, "id"> {
  const address = hostOf(byId<HTMLInputElement>("bm-address").value);
  const name = byId<HTMLInputElement>("bm-name").value.trim() || address;
  return { name, address, folder: byId<HTMLSelectElement>("bm-folder").value };
}

function saveEditor(event: SubmitEvent): void {
  event.preventDefault();
  const draft = readEditor();
  if (!isI2pAddress(draft.address)) {
    setAddressError("This is not an I2P address. It must end in .i2p.");
    byId("bm-address").focus();
    return;
  }
  const id = editingId;
  bookmarks = id
    ? bookmarks.map((b) => (b.id === id ? { ...draft, id } : b))
    : [...bookmarks, { ...draft, id: `bm-${nextId++}` }];
  byId<HTMLDialogElement>("editor").close();
  render();
  announce(status(), `Saved ${draft.name}.`);
}

function deleteBookmark(id: string): void {
  const doomed = bookmarks.find((b) => b.id === id);
  bookmarks = bookmarks.filter((b) => b.id !== id);
  render();
  announce(status(), doomed ? `Deleted ${doomed.name}.` : "Deleted.");
}

function onListClick(event: MouseEvent): void {
  const button = (event.target as Element).closest<HTMLButtonElement>("button[data-action]");
  const id = button?.dataset.id;
  if (!id) return;
  if (button.dataset.action === "delete") {
    deleteBookmark(id);
    return;
  }
  openEditor(bookmarks.find((b) => b.id === id) ?? null);
}

function exportBookmarks(): void {
  const data = bookmarks.map(({ name, address, folder }) => ({ name, address, folder }));
  const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
  const link = document.createElement("a");
  link.href = URL.createObjectURL(blob);
  link.download = "eepview-bookmarks.json";
  link.click();
  URL.revokeObjectURL(link.href);
  announce(status(), `Exported ${countLabel(bookmarks.length)}.`);
}

function toBookmark(entry: unknown): Bookmark | null {
  const record = entry as Partial<Bookmark> | null;
  if (typeof record?.address !== "string" || !isI2pAddress(record.address)) return null;
  const folder = FOLDERS.includes(record.folder ?? "") ? (record.folder ?? UNFILED) : UNFILED;
  const name = typeof record.name === "string" ? record.name : hostOf(record.address);
  return { id: `bm-${nextId++}`, name, address: hostOf(record.address), folder };
}

async function importBookmarks(file: File): Promise<void> {
  const parsed: unknown = JSON.parse(await file.text());
  const entries = Array.isArray(parsed) ? parsed : [];
  const added = entries.map(toBookmark).filter((b): b is Bookmark => b !== null);
  bookmarks = [...bookmarks, ...added];
  render();
  const skipped = entries.length - added.length;
  announce(status(), `Imported ${countLabel(added.length)}. Skipped ${skipped} that were not I2P.`);
}

function onImportChosen(event: Event): void {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = "";
  if (!file) return;
  importBookmarks(file).catch(() => announce(status(), "That file is not a bookmarks export."));
}

function wire(): void {
  byId("folders").addEventListener("click", onListClick);
  byId("add-btn").addEventListener("click", () => openEditor(null));
  byId("export-btn").addEventListener("click", exportBookmarks);
  byId("import-btn").addEventListener("click", () => byId("import-file").click());
  byId("import-file").addEventListener("change", onImportChosen);
  byId<HTMLFormElement>("editor-form").addEventListener("submit", saveEditor);
  byId("editor-cancel").addEventListener("click", () => byId<HTMLDialogElement>("editor").close());
}

render();
wire();
if (window.location.hash === "#add") openEditor(null);
