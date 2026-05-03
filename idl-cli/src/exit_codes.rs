//! Exit code constants per clig.dev spec.
//!
//! 0: Success
//! 1: Generic error
//! 2: Invalid usage / misuse of command
//! 3: Resource not found
//! 4: Conflict (e.g., proposal already accepted)

use std::process::ExitCode;

pub const SUCCESS: u8 = 0;
pub const ERROR: u8 = 1;
pub const USAGE_ERROR: u8 = 2;
#[allow(dead_code)]
pub const NOT_FOUND: u8 = 3;
pub const CONFLICT: u8 = 4;

pub fn success() -> ExitCode {
    ExitCode::from(SUCCESS)
}

pub fn error() -> ExitCode {
    ExitCode::from(ERROR)
}

pub fn usage_error() -> ExitCode {
    ExitCode::from(USAGE_ERROR)
}

#[allow(dead_code)]
pub fn not_found() -> ExitCode {
    ExitCode::from(NOT_FOUND)
}

pub fn conflict() -> ExitCode {
    ExitCode::from(CONFLICT)
}
