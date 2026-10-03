// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT
import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";

const config = JSON.parse(readFileSync(new URL("./screenshots.json", import.meta.url), "utf8"));
const baseUrl = process.env.SCREENSHOT_BASE ?? "";
const debugUrl = `http://127.0.0.1:${config.debugPort}`;

const CHROME_PATHS = {
  darwin: "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  linux: "google-chrome",
  win32: "chrome.exe",
};

function launchChrome(profile) {
  const binary = process.env.CHROME ?? CHROME_PATHS[process.platform];
  const args = [
    "--headless=new",
    "--hide-scrollbars",
    "--no-first-run",
    `--remote-debugging-port=${config.debugPort}`,
    `--user-data-dir=${profile}`,
    "about:blank",
  ];
  return spawn(binary, args, { stdio: "ignore" });
}

async function pageSocketUrl() {
  for (let tries = 0; tries < config.startupTries; tries += 1) {
    const targets = await fetch(`${debugUrl}/json/list`).then(
      (r) => r.json(),
      () => [],
    );
    const page = targets.find((t) => t.type === "page");
    if (page) return page.webSocketDebuggerUrl;
    await sleep(config.startupDelayMs);
  }
  throw new Error("Chrome did not open a page target");
}

function connect(url) {
  const socket = new WebSocket(url);
  const pending = new Map();
  const listeners = new Map();
  let nextId = 0;
  socket.addEventListener("message", ({ data }) => {
    const message = JSON.parse(data);
    if (message.id !== undefined) pending.get(message.id)?.(message);
    else listeners.get(message.method)?.(message.params);
  });
  const send = (method, params = {}) =>
    new Promise((resolve, reject) => {
      nextId += 1;
      pending.set(nextId, (m) =>
        m.error ? reject(new Error(m.error.message)) : resolve(m.result),
      );
      socket.send(JSON.stringify({ id: nextId, method, params }));
    });
  const once = (method) => new Promise((resolve) => listeners.set(method, resolve));
  const opened = new Promise((resolve) => socket.addEventListener("open", resolve));
  return opened.then(() => ({ send, once, close: () => socket.close() }));
}

async function evaluate(cdp, expression) {
  const { result } = await cdp.send("Runtime.evaluate", { expression, returnByValue: true });
  return result.value;
}

async function prepare(cdp, shot, theme) {
  await cdp.send("Emulation.setDeviceMetricsOverride", {
    ...config.viewport,
    deviceScaleFactor: shot.scale ?? 1,
    mobile: false,
  });
  await cdp.send("Emulation.setEmulatedMedia", {
    features: [{ name: "prefers-color-scheme", value: theme }],
  });
  const loaded = cdp.once("Page.loadEventFired");
  await cdp.send("Page.navigate", {
    url: `${baseUrl}/src/ui/${shot.page}?${shot.query}&theme=${theme}`,
  });
  await loaded;
  await sleep(config.settleMs);
}

async function click(cdp, selector) {
  await evaluate(cdp, `document.querySelector(${JSON.stringify(selector)}).click()`);
  await sleep(config.clickSettleMs);
}

async function clipFor(cdp, shot) {
  if (shot.rect) return { ...shot.rect, scale: 1 };
  if (!shot.element) return undefined;
  const { selector, pad } = shot.element;
  const box = await evaluate(
    cdp,
    `JSON.stringify(document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect())`,
  );
  const { x, y, width, height } = JSON.parse(box);
  return { x: x - pad, y: y - pad, width: width + 2 * pad, height: height + 2 * pad, scale: 1 };
}

async function capture(cdp, shot, theme) {
  await prepare(cdp, shot, theme);
  if (shot.click) await click(cdp, shot.click);
  const clip = await clipFor(cdp, shot);
  const { data } = await cdp.send("Page.captureScreenshot", {
    format: "png",
    ...(clip && { clip }),
  });
  const file = join(config.outDir, `${shot.name}-${theme}${shot.suffix ?? ""}.png`);
  writeFileSync(file, Buffer.from(data, "base64"));
  console.log(file);
}

async function captureAll(cdp) {
  await cdp.send("Page.enable");
  await cdp.send("Network.setUserAgentOverride", { userAgent: config.userAgent });
  for (const theme of config.themes) {
    for (const shot of config.shots) await capture(cdp, shot, theme);
  }
}

async function main() {
  if (!baseUrl) throw new Error("Set SCREENSHOT_BASE to the preview server URL");
  const profile = mkdtempSync(join(tmpdir(), "eepview-shots-"));
  const chrome = launchChrome(profile);
  try {
    const cdp = await connect(await pageSocketUrl());
    await captureAll(cdp);
    cdp.close();
  } finally {
    const exited = once(chrome, "exit");
    chrome.kill();
    await exited;
    rmSync(profile, { recursive: true, force: true, maxRetries: config.cleanupRetries });
  }
}

await main();
