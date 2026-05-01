//! `idl change <subcmd>` — P0.4 change-folder scaffolding.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{bail, Context, Result};

use crate::commands::init::scaffold_change;

const DECISION_TEMPLATE: &str = "# Decision\n\n\
- **Status:** draft\n\
- **Context:** Why is this change being proposed?\n\
- **Decision:** What is being changed in the intent graph?\n\
- **Consequences:** What does this enable / break / require?\n";

pub fn new(slug: String) -> Result<ExitCode> {
    if !is_valid_slug(&slug) {
        bail!("invalid slug `{slug}` — use kebab-case [a-z0-9-]");
    }

    let intent = locate_intent_dir()?;
    let next_id = next_change_number(&intent)?;
    let folder_name = format!("{next_id:04}-{slug}");

    scaffold_change(&intent, &folder_name, "draft", DECISION_TEMPLATE)
        .with_context(|| format!("scaffold change {folder_name}"))?;

    println!(
        "✓ change scaffolded: intent/changes/{}/\n  state.json (draft)\n  intent-delta.idl\n  decisions.md",
        folder_name
    );
    Ok(ExitCode::from(0))
}

pub fn list() -> Result<ExitCode> {
    let intent = locate_intent_dir()?;
    let changes = intent.join("changes");
    if !changes.is_dir() {
        println!("no changes/ directory yet");
        return Ok(ExitCode::from(0));
    }
    let mut entries: Vec<_> = std::fs::read_dir(&changes)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    if entries.is_empty() {
        println!("no changes yet");
        return Ok(ExitCode::from(0));
    }

    println!("changes:");
    for e in entries {
        let name = e.file_name().to_string_lossy().into_owned();
        let state = std::fs::read_to_string(e.path().join("state.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("state").and_then(|x| x.as_str()).map(|s| s.to_string()))
            .unwrap_or_else(|| "?".into());
        println!("  {name}  [{state}]");
    }
    Ok(ExitCode::from(0))
}

pub fn stub(verb: &str, id: &str) -> Result<ExitCode> {
    println!(
        "TODO: implement `idl change {verb} {id}` after extractor / accept pipeline lands."
    );
    Ok(ExitCode::from(0))
}

pub fn stub_with(detail: &str) -> Result<ExitCode> {
    println!("TODO: implement `idl change {detail}` after extractor / accept pipeline lands.");
    Ok(ExitCode::from(0))
}

fn locate_intent_dir() -> Result<PathBuf> {
    let candidates = [PathBuf::from("intent"), PathBuf::from(".")];
    for c in &candidates {
        if c.is_dir() {
            // Prefer ./intent if present.
            if c.file_name().map(|n| n == "intent").unwrap_or(false) {
                return Ok(c.clone());
            }
        }
    }
    let intent = PathBuf::from("intent");
    if intent.is_dir() {
        return Ok(intent);
    }
    bail!("no `intent/` directory in cwd — run `idl init --greenfield` first")
}

fn next_change_number(intent: &std::path::Path) -> Result<u32> {
    let changes = intent.join("changes");
    if !changes.is_dir() {
        return Ok(1);
    }
    let mut max = 0u32;
    for e in std::fs::read_dir(&changes)? {
        let e = e?;
        if !e.path().is_dir() {
            continue;
        }
        let name = e.file_name();
        let name = name.to_string_lossy();
        let prefix: String = name.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(n) = prefix.parse::<u32>() {
            max = max.max(n);
        }
    }
    Ok(max + 1)
}

fn is_valid_slug(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !s.starts_with('-')
        && !s.ends_with('-')
}
