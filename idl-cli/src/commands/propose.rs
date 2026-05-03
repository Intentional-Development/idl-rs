//! `idl propose` — manage proposals (create, list, accept, reject).

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::json;

use crate::{exit_codes, output};
use idl_proposals::{
    accept_proposal_safe, audit_log, find_proposal, generate_proposal_id, list_proposals,
    locate_changes_dir, DiffOp, Proposal, ProposalStatus,
};

#[derive(Serialize)]
struct ProposalListItem {
    id: String,
    status: String,
    title: String,
    target_graph: String,
    created_at: String,
    source: String,
}

/// Create a new proposal from diff ops JSON file.
pub fn create(
    title: String,
    target_graph: PathBuf,
    ops_file: PathBuf,
    dry_run: bool,
    ctx: &output::OutputContext,
) -> Result<ExitCode> {
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

    // 3. Generate slug from title
    let slug = title
        .to_lowercase()
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect::<String>();

    // 4. Create proposal
    let id = generate_proposal_id(&slug);
    let target_graph_str = target_graph
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("invalid target graph path"))?
        .to_string();

    let author = "cli-user".to_string();
    let proposal = Proposal::new(
        id.clone(),
        author.clone(),
        target_graph_str.clone(),
        Some(title.clone()),
        diff_ops.clone(),
    );

    if dry_run {
        ctx.info("(dry-run) Would create proposal:");
        if ctx.json_mode {
            ctx.json(&proposal)?;
        } else {
            ctx.stdout(&format!("  id: {}", id));
            ctx.stdout(&format!("  title: {}", title));
            ctx.stdout(&format!("  target: {}", target_graph_str));
            ctx.stdout(&format!("  ops: {}", diff_ops.len()));
        }
        return Ok(exit_codes::success());
    }

    // 5. Save proposal to changes/proposals/<id>.json
    let changes = locate_changes_dir()?;
    let proposals_dir = changes.join("proposals");
    std::fs::create_dir_all(&proposals_dir)
        .with_context(|| format!("create proposals directory at {}", proposals_dir.display()))?;

    let proposal_path = proposals_dir.join(format!("{}.json", id));
    proposal.save(&proposal_path)?;

    // 6. Append to AI audit log (.idl/ai-run.jsonl)
    idl_proposals::ai_audit_log(
        "create",
        &id,
        &author,
        Some("cli"),
        None,
        Some(json!({"title": title.clone(), "target": target_graph_str.clone(), "ops_count": diff_ops.len()})),
    )?;

    // 7. Append to legacy audit log (changes/audit.jsonl)
    audit_log(
        "create",
        &id,
        &author,
        Some("cli"),
        Some(json!({"title": title, "target": target_graph_str, "ops_count": diff_ops.len()})),
    )?;

    // 8. Print proposal id on success (machine-readable output to stdout)
    if ctx.json_mode {
        ctx.json(&json!({"id": id, "status": "created"}))?;
    } else {
        ctx.stdout(&id);
    }

    Ok(exit_codes::success())
}

/// List proposals, optionally filtered by status.
pub fn list(status_filter: Option<String>, ctx: &output::OutputContext) -> Result<ExitCode> {
    let filter = status_filter
        .as_ref()
        .map(|s| match s.as_str() {
            "pending" => Ok(ProposalStatus::Pending),
            "accepted" => Ok(ProposalStatus::Accepted),
            "rejected" => Ok(ProposalStatus::Rejected),
            _ => bail!(
                "invalid status '{}' (must be pending, accepted, or rejected)",
                s
            ),
        })
        .transpose()?;

    let proposals = list_proposals(filter)?;

    if proposals.is_empty() {
        if ctx.json_mode {
            ctx.json(&json!({"proposals": []}))?;
        } else if let Some(ref s) = status_filter {
            ctx.stdout(&format!("no {} proposals", s));
        } else {
            ctx.stdout("no proposals found");
        }
        return Ok(exit_codes::success());
    }

    if ctx.json_mode {
        let items: Vec<ProposalListItem> = proposals
            .iter()
            .map(|(_, p)| ProposalListItem {
                id: p.id.clone(),
                status: format!("{:?}", p.status).to_lowercase(),
                title: p.rationale.clone().unwrap_or_else(|| "(no title)".to_string()),
                target_graph: p.target_graph.clone(),
                created_at: p.created_at.to_rfc3339(),
                source: "cli".to_string(),
            })
            .collect();
        ctx.json(&json!({"proposals": items}))?;
    } else {
        // Human-readable table
        ctx.stdout(&format!(
            "{:40} {:10} {:40} {:25} {:20} {:8}",
            "id", "status", "title", "target", "created_at", "source"
        ));
        ctx.stdout(&"-".repeat(153));

        for (_, proposal) in proposals {
            let title = proposal.rationale.as_deref().unwrap_or("(no title)");
            let title_short = if title.len() > 40 {
                format!("{}...", &title[..37])
            } else {
                title.to_string()
            };

            let target_short = if proposal.target_graph.len() > 25 {
                format!(
                    "...{}",
                    &proposal.target_graph[proposal.target_graph.len() - 22..]
                )
            } else {
                proposal.target_graph.clone()
            };

            let created = proposal.created_at.format("%Y-%m-%d %H:%M:%S");
            let source = "cli";

            ctx.stdout(&format!(
                "{:40} {:10} {:40} {:25} {:20} {:8}",
                &proposal.id[..40.min(proposal.id.len())],
                format!("{:?}", proposal.status).to_lowercase(),
                title_short,
                target_short,
                created,
                source
            ));
        }
    }

    Ok(exit_codes::success())
}

/// Accept a proposal and apply it to the target graph.
pub fn accept(id: String, dry_run: bool, ctx: &output::OutputContext) -> Result<ExitCode> {
    // Find proposal first to validate it exists
    let (_, proposal) = find_proposal(&id)?;

    if proposal.status != ProposalStatus::Pending {
        ctx.warn(&format!(
            "proposal {} is not pending (status: {:?})",
            proposal.id, proposal.status
        ));
        return Ok(exit_codes::conflict());
    }

    if dry_run {
        ctx.info(&format!("(dry-run) Would accept proposal: {}", id));
        if ctx.json_mode {
            ctx.json(&json!({
                "id": proposal.id,
                "status": "would-accept",
                "target_graph": proposal.target_graph,
                "ops_count": proposal.diff_ops.len()
            }))?;
        } else {
            ctx.stdout(&format!("  id: {}", proposal.id));
            ctx.stdout(&format!("  target: {}", proposal.target_graph));
            ctx.stdout(&format!("  ops: {}", proposal.diff_ops.len()));
        }
        return Ok(exit_codes::success());
    }

    let hash = accept_proposal_safe(&id, "cli-user", Some("cli"))?;

    if ctx.json_mode {
        ctx.json(&json!({"id": id, "status": "accepted", "graph_hash": hash}))?;
    } else {
        ctx.stdout(&format!("Accepted: {}", id));
        ctx.info(&format!("  graph hash: {}", hash));
    }

    Ok(exit_codes::success())
}

/// Reject a proposal with a reason.
pub fn reject(
    id: String,
    reason: String,
    dry_run: bool,
    ctx: &output::OutputContext,
) -> Result<ExitCode> {
    // 1. Find proposal
    let (proposal_path, mut proposal) = find_proposal(&id)?;

    if proposal.status != ProposalStatus::Pending {
        ctx.warn(&format!(
            "proposal {} is not pending (status: {:?})",
            proposal.id, proposal.status
        ));
    }

    if dry_run {
        ctx.info(&format!("(dry-run) Would reject proposal: {}", id));
        if ctx.json_mode {
            ctx.json(&json!({
                "id": proposal.id,
                "status": "would-reject",
                "reason": reason
            }))?;
        } else {
            ctx.stdout(&format!("  id: {}", proposal.id));
            ctx.stdout(&format!("  reason: {}", reason));
        }
        return Ok(exit_codes::success());
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

    if ctx.json_mode {
        ctx.json(&json!({"id": proposal.id, "status": "rejected"}))?;
    } else {
        ctx.stdout(&format!("Rejected: {}", proposal.id));
    }

    Ok(exit_codes::success())
}
