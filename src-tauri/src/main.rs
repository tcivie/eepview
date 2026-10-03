//! The eepview binary.

// No console window next to the app on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fmt::Display;
use std::process::ExitCode;

fn main() -> ExitCode {
    exit_code(eepview_lib::run())
}

/// The exit code of a run: 1, with the error on stderr, when the app could not start.
fn exit_code<E: Display>(result: Result<(), E>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("eepview: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_start_exits_with_1() {
        assert_eq!(exit_code::<String>(Ok(())), ExitCode::SUCCESS);
        assert_eq!(exit_code(Err("no web view")), ExitCode::FAILURE);
    }
}
