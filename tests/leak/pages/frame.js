// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// The cross-origin frame: WebRTC and a loopback fetch from a second .i2p origin.
const { cfg, report, probe, webrtc } = window.leak;

// Tell the page when every probe has reported, so the page never navigates away first.
Promise.all([
  report("frame_js", "on"),
  probe("frame_loopback", `http://127.0.0.1:${cfg.canary}/frame-loopback`),
  webrtc("frame_").catch((error) => report("frame_rtc_error", String(error))),
]).then(() => window.parent.postMessage("leak-frame-done", "*"));
