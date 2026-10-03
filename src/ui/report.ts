import "./boot.ts";
import { byId } from "./dom.ts";
import { call, errorText } from "./ipc.ts";
import { BROWSER_NOTE, openedMessage, PLACEHOLDER, reportKind } from "./lib/report-page.ts";

const PREVIEW_DELAY_MS = 300;

const kind = reportKind(window.location.search);
const description = byId<HTMLTextAreaElement>("report-description");
const includeLog = byId<HTMLInputElement>("report-include-log");
const preview = byId<HTMLTextAreaElement>("report-preview");
const status = byId("report-status");
const button = byId<HTMLButtonElement>("report-open");

let timer = 0;

function inputs(): { kind: string; description: string; includeLog: boolean } {
  return { kind, description: description.value, includeLog: includeLog.checked };
}

function showPreview(): void {
  call("report_preview", inputs())
    .then((text) => {
      preview.value = text;
    })
    .catch((error) => {
      status.textContent = errorText(error);
    });
}

function previewLater(): void {
  window.clearTimeout(timer);
  timer = window.setTimeout(showPreview, PREVIEW_DELAY_MS);
}

function openIssue(event: Event): void {
  event.preventDefault();
  window.clearTimeout(timer);
  button.disabled = true;
  call("report_open", inputs())
    .then((result) => {
      status.textContent = openedMessage(result);
    })
    .catch((error) => {
      status.textContent = errorText(error);
    })
    .finally(() => {
      button.disabled = false;
    });
}

description.placeholder = PLACEHOLDER;
byId("report-note").textContent = BROWSER_NOTE;
description.addEventListener("input", previewLater);
includeLog.addEventListener("change", showPreview);
byId("report-form").addEventListener("submit", openIssue);
showPreview();
