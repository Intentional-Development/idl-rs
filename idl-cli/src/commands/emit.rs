//! `idl emit` — legacy passthrough preserved from prior CLI version.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use tracing::{info, warn};

pub fn run(idl_dir: PathBuf, output: PathBuf, target: String) -> Result<ExitCode> {
    info!(
        "Emitting {} code from {} to {}",
        target,
        idl_dir.display(),
        output.display()
    );

    let idl_files = crate::commands::parse::find_idl_files(&idl_dir)?;
    info!("Found {} IDL files", idl_files.len());

    for file in &idl_files {
        let content = std::fs::read_to_string(file)
            .with_context(|| format!("Failed to read {}", file.display()))?;
        match idl_core::parse_idl(&content) {
            Ok(doc) => info!("✓ Parsed {}: {} blocks", file.display(), doc.blocks.len()),
            Err(e) => warn!("✗ Failed to parse {}: {}", file.display(), e),
        }
    }

    warn!("⚠️  Emit command not yet implemented (port pending in Wave 8).");
    Ok(ExitCode::from(0))
}
