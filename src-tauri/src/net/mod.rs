// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Everything that opens a socket lives here (ADR 0001 rule 2).
//!
//! - [`host`]: the one I2P host predicate.
//! - [`loopback`]: socket addresses that can only point at this machine.
//! - [`verify`]: VERIFY of the router proxy, the only source of a [`verify::VerifiedUpstream`].
//! - [`failures`]: the pages whose load failed, for history.
//! - [`gatekeeper`]: eepview's own proxy in front of the router (layer L1).
//! - [`http`]: message heads for the gatekeeper.
//! - [`rules`]: the engine request rules (layer L3).
//! - [`stats`]: router statistics from the router helper.
//! - `testing` (tests only): a fake router proxy, so tests outside `net/` open no socket.

pub mod failures;
pub mod gatekeeper;
pub mod host;
pub mod http;
pub mod loopback;
pub mod rules;
pub mod stats;
#[cfg(test)]
pub(crate) mod testing;
pub mod verify;
