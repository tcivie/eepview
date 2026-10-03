// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! eepview: a browser that opens I2P sites only.
//!
//! Pure modules (no Tauri runtime, unit-tested): [`core`], [`icons`], [`nav`], [`net`], [`session`],
//! [`tabs`], [`store`], [`suggest`], [`shortcuts`], [`layout`], [`hover`], [`types`].
//! Tauri glue: [`shell`].

pub mod core;
pub mod hover;
pub mod icons;
pub mod layout;
pub mod nav;
pub mod net;
pub mod session;
pub mod shell;
pub mod shortcuts;
pub mod store;
pub mod suggest;
pub mod tabs;
pub mod theme;
pub mod types;

/// Starts the eepview application and blocks until it exits.
///
/// # Errors
///
/// Fails when Tauri cannot start, for example when the system web view is missing.
pub fn run() -> tauri::Result<()> {
    shell::run()
}
