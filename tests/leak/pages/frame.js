// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// The cross-origin frame: WebRTC and a loopback fetch from a second .i2p origin.
const { cfg, origins, report, probe, webrtc } = window.leak;

// The done handshake: the page pings until it hears `leak-frame-done`. The frame answers each
// ping once its probes have reported, and says so once by itself when they finish. A message
// sent before the other side listens is then never the last one.
let finished = false;

function answer() {
  window.parent.postMessage("leak-frame-done", origins.site);
}

window.addEventListener("message", (event) => {
  if (event.origin !== origins.site || event.source !== window.parent) return;
  if (event.data === "leak-frame-ping" && finished) answer();
});

Promise.all([
  report("frame_js", "on"),
  probe("frame_loopback", `http://127.0.0.1:${cfg.canary}/frame-loopback`),
  webrtc("frame_").catch((error) => report("frame_rtc_error", String(error))),
]).then(() => {
  finished = true;
  answer();
});
