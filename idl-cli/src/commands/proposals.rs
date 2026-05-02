//! `idl proposals` — manage proposals (list, accept, reject).

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use serde_json::json;

use idl_graph::doc::GraphDoc;

use idl_proposals::{
    audit_log, find_proposal, list_proposals, sort_graph_nodes, accept_proposal_safe, ProposalStatus,
};

pub fn list() -> Result<ExitCode> {
    let proposals = list_proposals(None)?;

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
    // Use the safe accept function from the proposals library
    let hash = accept_proposal_safe(&id, "cli-user", Some("cli"))?;

    println!("✓ proposal accepted");
    println!("  graph hash: {}", hash);

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
        "cli-user",
        Some("cli"),
        Some(json!({"reason": reason})),
    )?;

    println!("✓ proposal rejected");

    Ok(ExitCode::from(0))
}
