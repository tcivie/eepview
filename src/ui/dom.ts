// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

export function byId<T extends HTMLElement>(id: string): T {
  const el = document.getElementById(id);
  if (!el) throw new Error(`Missing element #${id}`);
  return el as T;
}

export function all<T extends Element>(selector: string, root: ParentNode = document): T[] {
  return Array.from(root.querySelectorAll<T>(selector));
}

export function announce(region: HTMLElement, text: string): void {
  region.textContent = text;
}

export function cloneTemplate(id: string): HTMLElement {
  const template = byId<HTMLTemplateElement>(id);
  return template.content.firstElementChild?.cloneNode(true) as HTMLElement;
}

export function setText(root: ParentNode, selector: string, text: string): void {
  const el = root.querySelector(selector);
  if (el) el.textContent = text;
}
