// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// Every vector from ADR 0001. Each one must be stopped by eepview; none may reach a canary
// or a clearnet host. Navigation vectors run last, because a leak there leaves the page.
const { cfg, origins, report, probe, sleep, webrtc } = window.leak;

function websocket(key, url) {
  try {
    const socket = new WebSocket(url);
    socket.onopen = () => report(key, "open");
    socket.onerror = () => report(key, "error");
  } catch (error) {
    report(key, `throw ${error.name}`);
  }
}

function subresourceVectors() {
  websocket("ws_clearnet", "ws://ws.clear.example/");
  websocket("ws_loopback", `ws://127.0.0.1:${cfg.canary}/ws`);
  return [
    probe("fetch_loopback", `http://127.0.0.1:${cfg.canary}/fetch-loopback`),
    probe("fetch_localhost", `http://localhost:${cfg.canary}/fetch-localhost`),
    probe("fetch_lan", `http://${cfg.lan}/fetch-lan`),
    probe("fetch_clearnet_ip", "http://203.0.113.7/fetch-ip"),
    probe("fetch_clearnet_name", "http://fetch.clear.example/"),
    probe("fetch_redirect", "/redirect"),
    webrtc("").catch((error) => report("rtc_error", String(error))),
  ];
}

function popupVectors() {
  const popup = window.open("http://popup.clear.example/", "_blank");
  report("window_open", popup ? "returned a window" : "null");
  document.getElementById("blank-link").click();
}

// The frame says so when all its probes have reported (see frame.js). A page that waits for a
// fixed time instead can navigate away while a slow runner is still loading the frame's scripts.
// The harness sets the cap from EEPVIEW_EXIT_AFTER (harness.frame_wait_ms), so the vectors
// after the wait still run before the app quits.
const FRAME_DEADLINE_MS = Number(cfg.frameWait) || 15000;
// The vectors that main() does not await (WebSockets, images, link hints, the popups) get at
// least this long before the first navigation can cancel them.
const DWELL_MS = 4000;
const PING_MS = 250;

// Pings the frame until it answers, so neither side's message is lost when the other side
// is not listening yet. Only the frame's own window and origin count.
function frameDone() {
  const frame = document.getElementById("frame");
  return new Promise((resolve) => {
    const ping = setInterval(() => {
      frame.contentWindow?.postMessage("leak-frame-ping", origins.frame);
    }, PING_MS);
    window.addEventListener("message", (event) => {
      if (event.origin !== origins.frame || event.source !== frame.contentWindow) return;
      if (event.data !== "leak-frame-done") return;
      clearInterval(ping);
      resolve("done");
    });
  });
}

async function main() {
  const frame = frameDone();
  await report("js", "on");
  popupVectors();
  await Promise.all(subresourceVectors());
  await report("done", "1");
  // Fail closed: when the frame never signals, the page goes on after the deadline and the
  // harness fails the frame checks, because the frame's reports are missing.
  const [waited] = await Promise.all([
    Promise.race([frame, sleep(FRAME_DEADLINE_MS).then(() => "timeout")]),
    sleep(DWELL_MS),
  ]);
  await report("frame_wait", waited);
  await report("nav_form", "1"); // reported first: a slow or blocked navigation must not hide a skipped one
  document.getElementById("clearnet-form").submit();
  await sleep(3000);
  await report("nav_location", "1");
  window.location = "http://example.com/";
}

main().catch((error) => report("main_error", String(error)));
