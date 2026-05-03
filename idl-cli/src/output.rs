//! Output utilities for stdout/stderr discipline per clig.dev.
//!
//! Machine-readable data (esp. with --json) goes to stdout.
//! Human "chrome" (progress, warnings, context) goes to stderr.

use std::io::{self, Write};

/// Global output context. Set from CLI flags.
#[derive(Default)]
pub struct OutputContext {
    pub json_mode: bool,
    pub quiet: bool,
    pub no_input: bool,
}

impl OutputContext {
    pub fn new(json_mode: bool, quiet: bool, no_input: bool) -> Self {
        Self {
            json_mode,
            quiet,
            no_input,
        }
    }

    /// Write human-readable info to stderr (unless quiet).
    pub fn info(&self, msg: &str) {
        if !self.quiet && !self.json_mode {
            eprintln!("{}", msg);
        }
    }

    /// Write human-readable warning to stderr.
    pub fn warn(&self, msg: &str) {
        if !self.json_mode {
            eprintln!("warning: {}", msg);
        }
    }

    /// Write error to stderr.
    pub fn error(&self, msg: &str) {
        eprintln!("error: {}", msg);
    }

    /// Write machine-readable JSON to stdout.
    pub fn json<T: serde::Serialize>(&self, value: &T) -> io::Result<()> {
        println!("{}", serde_json::to_string_pretty(value)?);
        Ok(())
    }

    /// Write human-readable data to stdout (for non-JSON output).
    pub fn stdout(&self, msg: &str) {
        println!("{}", msg);
    }

    /// Check if we can prompt for input. Fail if --no-input is set.
    #[allow(dead_code)]
    pub fn require_input_ok(&self) -> Result<(), String> {
        if self.no_input {
            Err("interactive input required but --no-input is set".to_string())
        } else {
            Ok(())
        }
    }
}

/// Utility for prompting user (respects --no-input).
#[allow(dead_code)]
pub fn prompt(ctx: &OutputContext, message: &str) -> Result<String, String> {
    ctx.require_input_ok()?;
    eprint!("{}", message);
    io::stderr().flush().map_err(|e| e.to_string())?;
    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(|e| e.to_string())?;
    Ok(input.trim().to_string())
}
