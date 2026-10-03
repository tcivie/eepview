// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// Every vector from ADR 0001. Each one must be stopped by eepview; none may reach a canary
// or a clearnet host. Navigation vectors run last, because a leak there leaves the page.
const { cfg, report, probe, sleep, webrtc } = window.leak;

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

async function main() {
  await report("js", "on");
  popupVectors();
  await Promise.all(subresourceVectors());
  await report("done", "1");
  await sleep(4000); // let the frame finish before a navigation could unload the page
  document.getElementById("clearnet-form").submit();
  await sleep(3000);
  window.location = "http://example.com/";
}

main().catch((error) => report("main_error", String(error)));
