//! `idl extract` — scaffold; full impl lands in next pass.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;

pub fn run(
    source: Option<PathBuf>,
    output: Option<PathBuf>,
    language: Option<String>,
) -> Result<ExitCode> {
    println!(
        "TODO: extraction adapters land in next pass; see brownfield extraction docs.\n  source:   {}\n  output:   {}\n  language: {}",
        source.as_ref().map(|p| p.display().to_string()).unwrap_or("(unset)".into()),
        output.as_ref().map(|p| p.display().to_string()).unwrap_or("(unset)".into()),
        language.as_deref().unwrap_or("(unset)"),
    );
    Ok(ExitCode::from(0))
}
