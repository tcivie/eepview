// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! eepview: a browser that opens I2P sites only.
//!
//! Pure modules (no Tauri runtime, unit-tested): [`core`], [`icons`], [`nav`], [`net`],
//! [`session`], [`tabs`], [`store`], [`suggest`], [`shortcuts`], [`layout`], [`popup`], [`hover`],
//! [`types`], [`input`], [`context_menu`].
//! Tauri glue: [`shell`].

pub mod context_menu;
pub mod core;
pub mod hover;
pub mod icons;
pub mod input;
pub mod layout;
pub mod nav;
pub mod net;
pub mod popup;
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
