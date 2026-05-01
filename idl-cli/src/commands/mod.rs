//! Subcommand bodies. Each module exposes a single `run` (or set of
//! verbs) that returns `anyhow::Result<std::process::ExitCode>`.

pub mod change;
pub mod drift;
pub mod emit;
pub mod extract;
pub mod init;
pub mod parse;
pub mod validate;
pub mod validate_anchors;
pub mod validate_schema;
