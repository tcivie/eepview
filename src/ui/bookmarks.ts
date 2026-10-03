import "./boot.ts";
import type { Bookmark } from "./contract.ts";
import { announce, byId, cloneTemplate, setText } from "./dom.ts";
import { call, errorText, on } from "./ipc.ts";
import { displayUrl, eepsiteUrl, isI2pAddress } from "./lib/address.ts";
import { folderGroups, folderNames } from "./lib/bookmark-groups.ts";

const ADDRESS_HINT = "A .i2p name or a .b32.i2p address.";

type Draft = { url: string; title: string; folder: string | null };

let bookmarks: Bookmark[] = [];
let editing: Bookmark | null = null;

const status = (): HTMLElement => byId("bookmark-status");
const countLabel = (n: number): string => (n === 1 ? "1 site" : `${n} sites`);
const reportError = (error: unknown): void => announce(status(), errorText(error));
const labelOf = (b: Bookmark): string => b.title || displayUrl(b.url);

function fillRow(row: HTMLElement, bookmark: Bookmark): void {
  row.querySelector("a")?.setAttribute("href", bookmark.url);
  setText(row, ".row-name", labelOf(bookmark));
  setText(row, ".row-addr", displayUrl(bookmark.url));
  for (const button of row.querySelectorAll<HTMLButtonElement>("button[data-action]")) {
    const verb = button.dataset.action === "edit" ? "Edit" : "Delete";
    button.dataset.id = bookmark.id;
    button.setAttribute("aria-label", `${verb} ${labelOf(bookmark)}`);
    button.title = verb;
  }
}

function renderRow(bookmark: Bookmark): HTMLElement {
  const row = cloneTemplate("row-template");
  fillRow(row, bookmark);
  return row;
}

function renderFolder(name: string, label: string, items: Bookmark[]): HTMLElement {
  const section = cloneTemplate("folder-template");
  const headingId = `folder-${name || "none"}`.toLowerCase().replace(/[^a-z0-9-]/g, "-");
  section.setAttribute("aria-labelledby", headingId);
  section.querySelector(".folder-name")?.setAttribute("id", headingId);
  setText(section, ".folder-name", label);
  setText(section, ".folder-count", countLabel(items.length));
  section.querySelector(".rows")?.append(...items.map(renderRow));
  return section;
}

function render(): void {
  const groups = folderGroups(bookmarks);
  byId("folders").replaceChildren(...groups.map((g) => renderFolder(g.name, g.label, g.items)));
  const options = folderNames(bookmarks).map((name) => new Option(name, name));
  byId("folder-list").replaceChildren(...options);
}

function show(list: Bookmark[]): void {
  bookmarks = list;
  render();
}

function load(): void {
  call("bookmarks_list", {}).then(show).catch(reportError);
}

function setAddressError(message: string | null): void {
  const hint = byId("bm-address-hint");
  byId("bm-address").setAttribute("aria-invalid", message ? "true" : "false");
  hint.textContent = message ?? ADDRESS_HINT;
  hint.classList.toggle("field-error", message !== null);
}

function openEditor(bookmark: Bookmark | null): void {
  editing = bookmark;
  byId("editor-title").textContent = bookmark ? "Edit bookmark" : "Add bookmark";
  byId<HTMLInputElement>("bm-name").value = bookmark?.title ?? "";
  byId<HTMLInputElement>("bm-address").value = bookmark ? displayUrl(bookmark.url) : "";
  byId<HTMLInputElement>("bm-folder").value = bookmark?.folder ?? "";
  setAddressError(null);
  byId<HTMLDialogElement>("editor").showModal();
}

function readEditor(): Draft {
  const address = byId<HTMLInputElement>("bm-address").value.trim();
  const folder = byId<HTMLInputElement>("bm-folder").value.trim();
  const url = isI2pAddress(address) ? eepsiteUrl(address) : address;
  const title = byId<HTMLInputElement>("bm-name").value.trim() || displayUrl(url);
  return { url, title, folder: folder || null };
}

async function save(draft: Draft): Promise<void> {
  if (editing) await call("bookmark_update", { bookmark: { ...editing, ...draft } });
  else await call("bookmark_add", { bookmark: draft });
  byId<HTMLDialogElement>("editor").close();
  announce(status(), `Saved ${draft.title}.`);
  load();
}

function onSave(event: SubmitEvent): void {
  event.preventDefault();
  const draft = readEditor();
  if (isI2pAddress(draft.url)) {
    save(draft).catch(reportError);
    return;
  }
  setAddressError("This is not an I2P address. It must end in .i2p.");
  byId("bm-address").focus();
}

async function deleteBookmark(id: string): Promise<void> {
  const doomed = bookmarks.find((b) => b.id === id);
  await call("bookmark_remove", { id });
  announce(status(), `Deleted ${doomed ? labelOf(doomed) : "the bookmark"}.`);
  load();
}

function onListClick(event: MouseEvent): void {
  const button = (event.target as Element).closest<HTMLButtonElement>("button[data-action]");
  const id = button?.dataset.id;
  if (!id) return;
  if (button.dataset.action === "delete") deleteBookmark(id).catch(reportError);
  else openEditor(bookmarks.find((b) => b.id === id) ?? null);
}

async function exportBookmarks(): Promise<void> {
  const json = await call("bookmarks_export", {});
  const link = document.createElement("a");
  link.href = URL.createObjectURL(new Blob([json], { type: "application/json" }));
  link.download = "eepview-bookmarks.json";
  link.click();
  URL.revokeObjectURL(link.href);
  announce(status(), `Exported ${countLabel(bookmarks.length)}.`);
}

async function importFile(file: File): Promise<void> {
  const added = await call("bookmarks_import", { json: await file.text() });
  announce(status(), `Imported ${countLabel(added)}.`);
  load();
}

function onImportChosen(event: Event): void {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = "";
  if (file) importFile(file).catch(reportError);
}

function wire(): void {
  byId("folders").addEventListener("click", onListClick);
  byId("add-btn").addEventListener("click", () => openEditor(null));
  byId("export-btn").addEventListener("click", () => exportBookmarks().catch(reportError));
  byId("import-btn").addEventListener("click", () => byId("import-file").click());
  byId("import-file").addEventListener("change", onImportChosen);
  byId<HTMLFormElement>("editor-form").addEventListener("submit", onSave);
  byId("editor-cancel").addEventListener("click", () => byId<HTMLDialogElement>("editor").close());
  on("bookmarks-changed", load).catch(reportError);
}

wire();
load();
if (window.location.hash === "#add") openEditor(null);
