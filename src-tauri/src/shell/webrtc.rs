// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Layer L5 of ADR 0001: the init script that removes WebRTC in every frame. The tab
//! factory (`content.rs`) and the console factory (`console.rs`) both use it.

/// Removes the WebRTC constructors in every frame, before any page script runs.
pub const WEBRTC_OFF_SCRIPT: &str = r"
for (const name of ['RTCPeerConnection', 'webkitRTCPeerConnection', 'RTCDataChannel',
                    'RTCSessionDescription', 'RTCIceCandidate', 'RTCRtpSender',
                    'RTCRtpReceiver', 'RTCRtpTransceiver']) {
  try { Object.defineProperty(window, name, { value: undefined, writable: false, configurable: false }); }
  catch (e) {}
}
try {
  if (navigator.mediaDevices) {
    Object.defineProperty(navigator, 'mediaDevices', { value: undefined, writable: false, configurable: false });
  }
} catch (e) {}
";

/// The init script that removes WebRTC in every frame.
#[must_use]
pub fn webrtc_off() -> &'static str {
    WEBRTC_OFF_SCRIPT
}
