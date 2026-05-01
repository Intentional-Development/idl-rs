//! `idl drift` — legacy passthrough preserved from prior CLI version.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use tracing::{info, warn};

pub fn run(
    idl_dir: PathBuf,
    generated: PathBuf,
    compare: String,
    language: String,
) -> Result<ExitCode> {
    info!(
        "Detecting drift between {} and {} (mode: {}, language: {})",
        idl_dir.display(),
        generated.display(),
        compare,
        language
    );
    warn!("⚠️  Drift command not yet implemented (port pending in Wave 8).");
    Ok(ExitCode::from(0))
}
