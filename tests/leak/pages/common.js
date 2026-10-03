// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

// Shared by the page and the cross-origin frame. Every report goes back to /report on the
// same .i2p host, so it travels through the gatekeeper like any page request.
// Wrapped so its names do not clash with the page script in the shared global scope.
(() => {
  const cfg = document.body.dataset;

  const report = (key, value) =>
    fetch(`/report?k=${key}&v=${encodeURIComponent(value)}`).catch(() => undefined);

  const probe = (key, url) =>
    fetch(url, { mode: "no-cors", signal: AbortSignal.timeout(4000) })
      .then(() => report(key, "resolved"))
      .catch((error) => report(key, `rejected ${error.name}`));

  // The two .i2p origins of the test site (servers.SITE_HOSTS). The done handshake between
  // the page and the frame accepts messages only from these.
  const origins = { site: "http://leaktest.i2p", frame: "http://frame.leaktest.i2p" };

  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

  async function webrtc(prefix) {
    await report(`${prefix}rtc_type`, typeof window.RTCPeerConnection);
    if (typeof window.RTCPeerConnection !== "function") {
      return;
    }
    const stun = [`stun:127.0.0.1:${cfg.udp}`, `stun:${cfg.udplan}`];
    const pc = new RTCPeerConnection({ iceServers: [{ urls: stun }] });
    const found = [];
    pc.onicecandidate = (event) => event.candidate && found.push(event.candidate.candidate);
    pc.createDataChannel("leak");
    await pc.setLocalDescription(await pc.createOffer());
    await sleep(3000);
    pc.close();
    await report(`${prefix}rtc_candidates`, JSON.stringify(found));
  }

  window.leak = { cfg, origins, report, probe, sleep, webrtc };
})();
