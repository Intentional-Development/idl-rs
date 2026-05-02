//! `idl propose` — create a new proposal.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use serde_json::Value;

use crate::proposals::{
    audit_log, generate_proposal_id, locate_changes_dir, DiffOp, Proposal,
};

pub fn run(graph_path: PathBuf, change_spec: PathBuf) -> Result<ExitCode> {
    // 1. Read the change spec
    let spec_content = std::fs::read_to_string(&change_spec)
        .with_context(|| format!("read change spec {}", change_spec.display()))?;
    let spec: Value = serde_json::from_str(&spec_content)
        .with_context(|| format!("parse change spec {}", change_spec.display()))?;

    // 2. Extract fields from spec
    let author = spec
        .get("author")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("change spec missing 'author' field"))?
        .to_string();

    let slug = spec
        .get("slug")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("change spec missing 'slug' field"))?;

    let rationale = spec
        .get("rationale")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let diff_ops_json = spec
        .get("diff_ops")
        .ok_or_else(|| anyhow::anyhow!("change spec missing 'diff_ops' field"))?;

    let diff_ops: Vec<DiffOp> = serde_json::from_value(diff_ops_json.clone())
        .context("parse diff_ops")?;

    if diff_ops.is_empty() {
        bail!("change spec must have at least one diff_op");
    }

    // 3. Validate target graph exists
    if !graph_path.exists() {
        bail!("target graph {} does not exist", graph_path.display());
    }

    // 4. Create proposal
    let id = generate_proposal_id(slug);
    let target_graph = graph_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("invalid graph path"))?
        .to_string();

    let proposal = Proposal::new(id.clone(), author.clone(), target_graph, rationale, diff_ops);

    // 5. Save proposal
    let changes = locate_changes_dir()?;
    let proposal_path = changes.join(format!("{}.proposal.json", id));
    proposal.save(&proposal_path)?;

    // 6. Log to audit trail
    audit_log("propose", &id, &author, None)?;

    println!("✓ proposal created: {}", proposal_path.display());
    println!("  id: {}", id);
    println!("  target: {}", proposal.target_graph);
    println!("  diff_ops: {}", proposal.diff_ops.len());

    Ok(ExitCode::from(0))
}
