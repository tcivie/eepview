import type { TabInfo } from "../contract.ts";
import { byId } from "../dom.ts";
import { call } from "../ipc.ts";
import { dropIndex, moveTarget, neighbour, tabMonogram, tabTitle } from "../lib/tab-strip.ts";
import { currentTabs } from "./state.ts";

const MIDDLE_BUTTON = 1;
const quiet = (): undefined => undefined;
let draggedId: number | null = null;

const strip = (): HTMLElement => byId("tabs");

function tabElement(target: EventTarget | null): HTMLElement | null {
  return (target as Element | null)?.closest<HTMLElement>(".tab") ?? null;
}

function idOf(el: HTMLElement | null): number | null {
  return el?.dataset.id ? Number(el.dataset.id) : null;
}

function fillTab(el: HTMLElement, tab: TabInfo): void {
  const title = tabTitle(tab);
  el.dataset.id = String(tab.id);
  el.setAttribute("aria-selected", String(tab.active));
  el.tabIndex = tab.active ? 0 : -1;
  el.title = title;
  el.classList.toggle("is-loading", tab.loading);
  el.classList.toggle("is-internal", tab.kind === "internal");
  const icon = el.querySelector(".tab-icon");
  if (icon) icon.textContent = tab.loading ? "" : tabMonogram(tab);
  const label = el.querySelector(".tab-title");
  if (label) label.textContent = title;
  el.querySelector(".tab-close")?.setAttribute("aria-label", `Close ${title}`);
}

function buildTab(tab: TabInfo): HTMLElement {
  const template = byId<HTMLTemplateElement>("tab-template");
  const el = template.content.firstElementChild?.cloneNode(true) as HTMLElement;
  fillTab(el, tab);
  return el;
}

export function renderTabs(tabs: TabInfo[]): void {
  const hadFocus = strip().contains(document.activeElement);
  strip().replaceChildren(...tabs.map(buildTab));
  const active = strip().querySelector<HTMLElement>('[aria-selected="true"]');
  active?.scrollIntoView({ block: "nearest", inline: "nearest" });
  if (hadFocus) active?.focus();
}

function closeTab(id: number): void {
  call("tab_close", { id }).catch(quiet);
}

function selectTab(id: number): void {
  call("tab_select", { id }).catch(quiet);
}

function onClick(event: MouseEvent): void {
  const id = idOf(tabElement(event.target));
  if (id === null) return;
  const onClose = (event.target as Element).closest(".tab-close") !== null;
  if (onClose) closeTab(id);
  else selectTab(id);
}

function onAuxClick(event: MouseEvent): void {
  const id = idOf(tabElement(event.target));
  if (event.button === MIDDLE_BUTTON && id !== null) closeTab(id);
}

function focusTab(id: number | undefined): void {
  if (id === undefined) return;
  strip().querySelector<HTMLElement>(`[data-id="${id}"]`)?.focus();
}

const KEY_STEPS: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1 };

function onKey(event: KeyboardEvent): void {
  const id = idOf(tabElement(event.target));
  if (id === null) return;
  const step = KEY_STEPS[event.key];
  if (step !== undefined) focusTab(neighbour(currentTabs(), id, step)?.id);
  if (event.key === "Enter" || event.key === " ") selectTab(id);
  if (event.key === "Delete") closeTab(id);
  if (step !== undefined || event.key === " ") event.preventDefault();
}

function onDragStart(event: DragEvent): void {
  draggedId = idOf(tabElement(event.target));
  event.dataTransfer?.setData("text/plain", String(draggedId));
  if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
}

function centres(): number[] {
  return [...strip().querySelectorAll<HTMLElement>(".tab")].map((el) => {
    const box = el.getBoundingClientRect();
    return box.left + box.width / 2;
  });
}

function onDrop(event: DragEvent): void {
  event.preventDefault();
  const from = currentTabs().findIndex((t) => t.id === draggedId);
  if (draggedId === null || from < 0) return;
  const to = moveTarget(from, dropIndex(event.clientX, centres()));
  if (to !== from) call("tab_move", { id: draggedId, index: to }).catch(quiet);
  draggedId = null;
}

function onWheel(event: WheelEvent): void {
  if (Math.abs(event.deltaY) <= Math.abs(event.deltaX)) return;
  strip().scrollLeft += event.deltaY;
  event.preventDefault();
}

export function wireTabs(): void {
  const el = strip();
  el.addEventListener("click", onClick);
  el.addEventListener("auxclick", onAuxClick);
  el.addEventListener("mousedown", (e) => {
    if (e.button === MIDDLE_BUTTON) e.preventDefault();
  });
  el.addEventListener("keydown", onKey);
  el.addEventListener("dragstart", onDragStart);
  el.addEventListener("dragover", (e) => e.preventDefault());
  el.addEventListener("drop", onDrop);
  el.addEventListener("wheel", onWheel, { passive: false });
  byId("new-tab").addEventListener("click", () => call("tab_new", {}).catch(quiet));
}
