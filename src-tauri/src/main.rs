// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The eepview binary.

// No console window next to the app on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;

use eepview_lib::diag::{self, Code, ErrorKind, Field};

fn main() -> ExitCode {
    exit_code(eepview_lib::run().map_err(|e| ErrorKind::from(&e)))
}

/// The exit code of a run: 1, with a `start-failed` event, when the app could not start.
fn exit_code(result: Result<(), ErrorKind>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(kind) => {
            diag::event(Code::StartFailed, &[Field::Error(kind)]);
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_start_exits_with_1() {
        assert_eq!(exit_code(Ok(())), ExitCode::SUCCESS);
        assert_eq!(exit_code(Err(ErrorKind::Webview)), ExitCode::FAILURE);
    }
}
