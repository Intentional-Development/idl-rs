//! `idl propose` — manage proposals (create, list, accept, reject).

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use serde_json::json;

use idl_proposals::{
    audit_log, find_proposal, generate_proposal_id, list_proposals, locate_changes_dir,
    accept_proposal_safe, DiffOp, Proposal, ProposalStatus,
};

/// Create a new proposal from diff ops JSON file.
pub fn create(title: String, target_graph: PathBuf, ops_file: PathBuf) -> Result<ExitCode> {
    // 1. Validate target graph exists
    if !target_graph.exists() {
        bail!("target graph {} does not exist", target_graph.display());
    }

    // 2. Read and parse ops file
    let ops_content = std::fs::read_to_string(&ops_file)
        .with_context(|| format!("read ops file {}", ops_file.display()))?;
    let diff_ops: Vec<DiffOp> = serde_json::from_str(&ops_content)
        .with_context(|| format!("parse diff_ops from {}", ops_file.display()))?;

    if diff_ops.is_empty() {
        bail!("ops file must contain at least one diff_op");
    }

    // 3. Validate ops file against proposal schema (basic check)
    // The DiffOp deserialization above already ensures schema compliance
    // since it uses the strongly-typed enum with serde tags

    // 4. Generate slug from title
    let slug = title
        .to_lowercase()
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect::<String>();

    // 5. Create proposal
    let id = generate_proposal_id(&slug);
    let target_graph_str = target_graph
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("invalid target graph path"))?
        .to_string();

    let author = "cli-user".to_string(); // Could be enhanced to read from git config
    let proposal = Proposal::new(
        id.clone(),
        author.clone(),
        target_graph_str.clone(),
        Some(title.clone()),
        diff_ops.clone(),
    );

    // 6. Save proposal to changes/proposals/<id>.json
    let changes = locate_changes_dir()?;
    let proposals_dir = changes.join("proposals");
    std::fs::create_dir_all(&proposals_dir)
        .with_context(|| format!("create proposals directory at {}", proposals_dir.display()))?;

    let proposal_path = proposals_dir.join(format!("{}.json", id));
    proposal.save(&proposal_path)?;

    // 7. Append to changes/audit.jsonl with source="cli"
    audit_log(
        "create",
        &id,
        &author,
        Some("cli"),
        Some(json!({"title": title, "target": target_graph_str, "ops_count": diff_ops.len()})),
    )?;

    // 8. Print proposal id on success
    println!("{}", id);

    Ok(ExitCode::from(0))
}

/// List proposals, optionally filtered by status.
pub fn list(status_filter: Option<String>) -> Result<ExitCode> {
    let filter = status_filter.as_ref().map(|s| match s.as_str() {
        "pending" => Ok(ProposalStatus::Pending),
        "accepted" => Ok(ProposalStatus::Accepted),
        "rejected" => Ok(ProposalStatus::Rejected),
        _ => bail!("invalid status '{}' (must be pending, accepted, or rejected)", s),
    }).transpose()?;

    let proposals = list_proposals(filter)?;

    if proposals.is_empty() {
        if let Some(ref s) = status_filter {
            println!("no {} proposals", s);
        } else {
            println!("no proposals found");
        }
        return Ok(ExitCode::from(0));
    }

    // Print header
    println!(
        "{:40} {:10} {:40} {:25} {:20} {:8}",
        "id", "status", "title", "target", "created_at", "source"
    );
    println!("{}", "-".repeat(153));

    for (_, proposal) in proposals {
        let title = proposal.rationale.as_deref().unwrap_or("(no title)");
        let title_short = if title.len() > 40 {
            format!("{}...", &title[..37])
        } else {
            title.to_string()
        };

        let target_short = if proposal.target_graph.len() > 25 {
            format!("...{}", &proposal.target_graph[proposal.target_graph.len() - 22..])
        } else {
            proposal.target_graph.clone()
        };

        let created = proposal.created_at.format("%Y-%m-%d %H:%M:%S");
        let source = "cli"; // Could be enhanced to track source from audit log

        println!(
            "{:40} {:10} {:40} {:25} {:20} {:8}",
            &proposal.id[..40.min(proposal.id.len())],
            format!("{:?}", proposal.status).to_lowercase(),
            title_short,
            target_short,
            created,
            source
        );
    }

    Ok(ExitCode::from(0))
}

/// Accept a proposal and apply it to the target graph.
pub fn accept(id: String) -> Result<ExitCode> {
    // Use the safe accept function from the proposals library with file-lock
    let hash = accept_proposal_safe(&id, "cli-user", Some("cli"))?;

    println!("Accepted: {}", id);
    println!("  graph hash: {}", hash);

    Ok(ExitCode::from(0))
}

/// Reject a proposal with a reason.
pub fn reject(id: String, reason: String) -> Result<ExitCode> {
    // 1. Find proposal
    let (proposal_path, mut proposal) = find_proposal(&id)?;

    if proposal.status != ProposalStatus::Pending {
        eprintln!(
            "warning: proposal {} is not pending (status: {:?})",
            proposal.id, proposal.status
        );
    }

    // 2. Update proposal status
    proposal.reject(reason.clone());
    proposal.save(&proposal_path)?;

    // 3. Log to audit trail with source="cli"
    audit_log(
        "reject",
        &proposal.id,
        "cli-user",
        Some("cli"),
        Some(json!({"reason": reason})),
    )?;

    println!("Rejected: {}", proposal.id);

    Ok(ExitCode::from(0))
}
