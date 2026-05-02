//! `idl proposals` — manage proposals (list, accept, reject).

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use serde_json::json;

use idl_graph::doc::GraphDoc;

use crate::proposals::{
    audit_log, find_proposal, list_proposals, sort_graph_nodes, ProposalStatus,
};

pub fn list() -> Result<ExitCode> {
    let proposals = list_proposals()?;

    if proposals.is_empty() {
        println!("no proposals found");
        return Ok(ExitCode::from(0));
    }

    println!("proposals:");
    for (path, proposal) in proposals {
        let status_icon = match proposal.status {
            ProposalStatus::Pending => "○",
            ProposalStatus::Accepted => "✓",
            ProposalStatus::Rejected => "✗",
        };
        println!(
            "  {} {} [{}] — {}",
            status_icon,
            proposal.id,
            format!("{:?}", proposal.status).to_lowercase(),
            path.display()
        );
        println!("      author: {}", proposal.author);
        println!("      target: {}", proposal.target_graph);
        println!("      ops: {}", proposal.diff_ops.len());
        if let Some(reason) = &proposal.rejection_reason {
            println!("      rejection: {}", reason);
        }
    }

    Ok(ExitCode::from(0))
}

pub fn accept(id: String) -> Result<ExitCode> {
    // 1. Find proposal
    let (proposal_path, mut proposal) = find_proposal(&id)?;

    if proposal.status != ProposalStatus::Pending {
        eprintln!(
            "warning: proposal {} is not pending (status: {:?})",
            proposal.id, proposal.status
        );
    }

    println!("accepting proposal: {}", proposal.id);
    println!("  target: {}", proposal.target_graph);
    println!("  ops: {}", proposal.diff_ops.len());

    // 2. Load target graph
    let graph_path = PathBuf::from(&proposal.target_graph);
    if !graph_path.exists() {
        anyhow::bail!("target graph {} does not exist", graph_path.display());
    }

    let graph_content = std::fs::read_to_string(&graph_path)
        .with_context(|| format!("read target graph {}", graph_path.display()))?;
    let mut graph: GraphDoc = serde_json::from_str(&graph_content)
        .with_context(|| format!("parse target graph {}", graph_path.display()))?;

    // 3. Apply diff ops
    proposal
        .apply(&mut graph)
        .context("apply diff ops to graph")?;

    // 4. Sort nodes deterministically
    sort_graph_nodes(&mut graph);

    // 5. Validate result (basic schema check)
    // For MVP, we just ensure it serializes correctly
    let updated_content = serde_json::to_string_pretty(&graph)
        .context("serialize updated graph")?;

    // 6. Write back to disk
    std::fs::write(&graph_path, updated_content)
        .with_context(|| format!("write updated graph to {}", graph_path.display()))?;

    // 7. Update proposal status
    proposal.accept();
    proposal.save(&proposal_path)?;

    // 8. Log to audit trail
    audit_log(
        "accept",
        &proposal.id,
        &proposal.author,
        Some(json!({"target": proposal.target_graph})),
    )?;

    println!("✓ proposal accepted and applied to {}", graph_path.display());

    Ok(ExitCode::from(0))
}

pub fn reject(id: String, reason: String) -> Result<ExitCode> {
    // 1. Find proposal
    let (proposal_path, mut proposal) = find_proposal(&id)?;

    if proposal.status != ProposalStatus::Pending {
        eprintln!(
            "warning: proposal {} is not pending (status: {:?})",
            proposal.id, proposal.status
        );
    }

    println!("rejecting proposal: {}", proposal.id);
    println!("  reason: {}", reason);

    // 2. Update proposal status
    proposal.reject(reason.clone());
    proposal.save(&proposal_path)?;

    // 3. Log to audit trail
    audit_log(
        "reject",
        &proposal.id,
        &proposal.author,
        Some(json!({"reason": reason})),
    )?;

    println!("✓ proposal rejected");

    Ok(ExitCode::from(0))
}
