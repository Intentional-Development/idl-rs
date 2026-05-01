//! `idl parse` — legacy passthrough preserved from prior CLI version.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use tracing::{info, warn};

pub fn run(path: PathBuf, json: bool) -> Result<ExitCode> {
    info!("Parsing IDL at {}", path.display());

    if path.is_file() {
        parse_file(&path, json)?;
    } else if path.is_dir() {
        for file in find_idl_files(&path)? {
            parse_file(&file, json)?;
        }
    } else {
        anyhow::bail!("Path does not exist: {}", path.display());
    }
    Ok(ExitCode::from(0))
}

fn parse_file(file: &PathBuf, json: bool) -> Result<()> {
    let content = std::fs::read_to_string(file)
        .with_context(|| format!("Failed to read {}", file.display()))?;
    match idl_core::parse_idl(&content) {
        Ok(doc) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&doc)?);
            } else {
                info!(
                    "✓ {} - version {}, {} blocks",
                    file.display(),
                    doc.version,
                    doc.blocks.len()
                );
            }
        }
        Err(e) => {
            warn!("✗ Failed to parse {}: {}", file.display(), e);
            return Err(e.into());
        }
    }
    Ok(())
}

pub(crate) fn find_idl_files(dir: &PathBuf) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(dir) {
        let entry = entry?;
        if entry.file_type().is_file() {
            if entry.path().extension().map(|e| e == "idl").unwrap_or(false) {
                files.push(entry.path().to_path_buf());
            }
        }
    }
    Ok(files)
}
