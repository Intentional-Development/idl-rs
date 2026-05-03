//! Proposal management — core logic for IDL proposal workflows.
//!
//! Proposals are structured change requests that live in `<repo>/changes/` as
//! `<timestamp>-<slug>.proposal.json` files. Each proposal contains:
//! - Metadata (id, author, target_graph, status, timestamps)
//! - A list of diff operations (add_dto, remove_dto, modify_dto_field, change_kind)
//! - Rationale for the change
//!
//! The workflow:
//! 1. Create a proposal file (via CLI or MCP)
//! 2. List proposals (by status or all)
//! 3. Accept a proposal — applies diff ops to target graph
//! 4. Reject a proposal — marks as rejected with reason
//!
//! All actions are logged to `<repo>/changes/audit.jsonl` with source attribution.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use idl_graph::doc::{GraphDoc, NodeDoc};

const PROPOSAL_VERSION: &str = "0.1.0";

/// A proposal — schema-versioned change request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub version: String,
    pub id: String,
    pub author: String,
    pub target_graph: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    pub diff_ops: Vec<DiffOp>,
    pub status: ProposalStatus,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProposalStatus {
    Pending,
    Accepted,
    Rejected,
}

/// Diff operation — minimal composable vocabulary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum DiffOp {
    AddDto { dto: NodeDoc },
    RemoveDto { node_id: String },
    ModifyDtoField {
        dto_id: String,
        field_name: String,
        action: FieldAction,
        #[serde(skip_serializing_if = "Option::is_none")]
        field_data: Option<Value>,
    },
    ChangeKind { node_id: String, new_kind: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldAction {
    Add,
    Remove,
    Update,
}

/// Audit log entry — one JSON object per line in `changes/audit.jsonl`.
#[derive(Debug, Serialize)]
pub struct AuditEntry {
    pub timestamp: DateTime<Utc>,
    pub action: String,
    pub proposal_id: String,
    pub author: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl Proposal {
    /// Create a new proposal.
    pub fn new(
        id: String,
        author: String,
        target_graph: String,
        rationale: Option<String>,
        diff_ops: Vec<DiffOp>,
    ) -> Self {
        Self {
            version: PROPOSAL_VERSION.to_string(),
            id,
            author,
            target_graph,
            rationale,
            diff_ops,
            status: ProposalStatus::Pending,
            created_at: Utc::now(),
            updated_at: None,
            rejection_reason: None,
        }
    }

    /// Load a proposal from disk.
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("read proposal {}", path.display()))?;
        let proposal: Proposal = serde_json::from_str(&content)
            .with_context(|| format!("parse proposal {}", path.display()))?;
        if proposal.version != PROPOSAL_VERSION {
            bail!(
                "unsupported proposal version {} (expected {})",
                proposal.version,
                PROPOSAL_VERSION
            );
        }
        Ok(proposal)
    }

    /// Save proposal to disk.
    pub fn save(&self, path: &Path) -> Result<()> {
        let content =
            serde_json::to_string_pretty(self).context("serialize proposal")?;
        fs::write(path, content)
            .with_context(|| format!("write proposal {}", path.display()))?;
        Ok(())
    }

    /// Apply this proposal's diff ops to a graph document.
    pub fn apply(&self, graph: &mut GraphDoc) -> Result<()> {
        for op in &self.diff_ops {
            match op {
                DiffOp::AddDto { dto } => {
                    // Check for duplicate
                    if graph.nodes.iter().any(|n| n.id == dto.id) {
                        bail!("node {} already exists", dto.id);
                    }
                    graph.nodes.push(dto.clone());
                }
                DiffOp::RemoveDto { node_id } => {
                    let idx = graph
                        .nodes
                        .iter()
                        .position(|n| n.id == *node_id)
                        .ok_or_else(|| anyhow!("node {} not found", node_id))?;
                    graph.nodes.remove(idx);
                }
                DiffOp::ModifyDtoField {
                    dto_id,
                    field_name,
                    action,
                    field_data,
                } => {
                    let node = graph
                        .nodes
                        .iter_mut()
                        .find(|n| n.id == *dto_id)
                        .ok_or_else(|| anyhow!("node {} not found", dto_id))?;

                    // Extract fields array from props.dto_props.fields
                    let dto_props = node
                        .props
                        .get_mut("dto_props")
                        .and_then(|v| v.as_object_mut())
                        .ok_or_else(|| {
                            anyhow!("node {} has no dto_props object", dto_id)
                        })?;

                    let fields = dto_props
                        .get_mut("fields")
                        .and_then(|v| v.as_array_mut())
                        .ok_or_else(|| {
                            anyhow!("node {} has no dto_props.fields array", dto_id)
                        })?;

                    match action {
                        FieldAction::Add => {
                            let field = field_data
                                .as_ref()
                                .ok_or_else(|| anyhow!("add requires field_data"))?;
                            // Check for duplicate
                            if fields.iter().any(|f| {
                                f.get("name")
                                    .and_then(|n| n.as_str())
                                    .map(|n| n == field_name)
                                    .unwrap_or(false)
                            }) {
                                bail!("field {} already exists in {}", field_name, dto_id);
                            }
                            fields.push(field.clone());
                        }
                        FieldAction::Remove => {
                            let idx = fields
                                .iter()
                                .position(|f| {
                                    f.get("name")
                                        .and_then(|n| n.as_str())
                                        .map(|n| n == field_name)
                                        .unwrap_or(false)
                                })
                                .ok_or_else(|| {
                                    anyhow!("field {} not found in {}", field_name, dto_id)
                                })?;
                            fields.remove(idx);
                        }
                        FieldAction::Update => {
                            let field = field_data
                                .as_ref()
                                .ok_or_else(|| anyhow!("update requires field_data"))?;
                            let idx = fields
                                .iter()
                                .position(|f| {
                                    f.get("name")
                                        .and_then(|n| n.as_str())
                                        .map(|n| n == field_name)
                                        .unwrap_or(false)
                                })
                                .ok_or_else(|| {
                                    anyhow!("field {} not found in {}", field_name, dto_id)
                                })?;
                            fields[idx] = field.clone();
                        }
                    }
                }
                DiffOp::ChangeKind { node_id, new_kind } => {
                    let node = graph
                        .nodes
                        .iter_mut()
                        .find(|n| n.id == *node_id)
                        .ok_or_else(|| anyhow!("node {} not found", node_id))?;
                    node.kind = new_kind.clone();
                }
            }
        }
        Ok(())
    }

    /// Mark as accepted.
    pub fn accept(&mut self) {
        self.status = ProposalStatus::Accepted;
        self.updated_at = Some(Utc::now());
    }

    /// Mark as rejected.
    pub fn reject(&mut self, reason: String) {
        self.status = ProposalStatus::Rejected;
        self.rejection_reason = Some(reason);
        self.updated_at = Some(Utc::now());
    }
}

/// Find the changes directory in the current repository.
pub fn locate_changes_dir() -> Result<PathBuf> {
    // Try ./changes first, then ./intent/changes
    let candidates = [PathBuf::from("changes"), PathBuf::from("intent/changes")];
    for c in &candidates {
        if c.exists() {
            return Ok(c.clone());
        }
    }
    // Create ./changes if neither exists
    let changes = PathBuf::from("changes");
    fs::create_dir_all(&changes)
        .with_context(|| format!("create changes directory at {}", changes.display()))?;
    Ok(changes)
}

/// List all proposals in the changes directory, optionally filtered by status.
pub fn list_proposals(status_filter: Option<ProposalStatus>) -> Result<Vec<(PathBuf, Proposal)>> {
    let changes = locate_changes_dir()?;
    let mut proposals = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    // Look in both changes/ and changes/proposals/ for backwards compatibility
    let search_dirs = vec![changes.clone(), changes.join("proposals")];

    for search_dir in search_dirs {
        if !search_dir.is_dir() {
            continue;
        }

        for entry in fs::read_dir(&search_dir)? {
            let entry = entry?;
            let path = entry.path();
            let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            
            // Skip non-JSON files and audit log
            if !path.is_file() 
                || path.extension().and_then(|e| e.to_str()) != Some("json")
                || filename == "audit.jsonl"
            {
                continue;
            }

            // Try to load as a proposal
            match Proposal::load(&path) {
                Ok(proposal) => {
                    // Skip if already seen (avoid duplicates from multiple dirs)
                    if seen_ids.contains(&proposal.id) {
                        continue;
                    }
                    seen_ids.insert(proposal.id.clone());

                    if let Some(ref filter) = status_filter {
                        if proposal.status == *filter {
                            proposals.push((path, proposal));
                        }
                    } else {
                        proposals.push((path, proposal));
                    }
                },
                Err(_e) => {
                    // Skip silently - might be a non-proposal JSON file
                }
            }
        }
    }

    proposals.sort_by(|a, b| a.1.created_at.cmp(&b.1.created_at));
    Ok(proposals)
}

/// Find a proposal by ID (accepts full ID or just the prefix).
pub fn find_proposal(id: &str) -> Result<(PathBuf, Proposal)> {
    let proposals = list_proposals(None)?;
    let matches: Vec<_> = proposals
        .into_iter()
        .filter(|(_, p)| p.id == id || p.id.starts_with(id))
        .collect();

    match matches.len() {
        0 => bail!("no proposal found with id {}", id),
        1 => Ok(matches.into_iter().next().unwrap()),
        _ => bail!(
            "ambiguous id {}: matches {} proposals",
            id,
            matches.len()
        ),
    }
}

/// Write an audit log entry to changes/audit.jsonl.
pub fn audit_log(
    action: &str,
    proposal_id: &str,
    author: &str,
    source: Option<&str>,
    details: Option<Value>,
) -> Result<()> {
    let changes = locate_changes_dir()?;
    let audit_path = changes.join("audit.jsonl");

    let entry = AuditEntry {
        timestamp: Utc::now(),
        action: action.to_string(),
        proposal_id: proposal_id.to_string(),
        author: author.to_string(),
        source: source.map(|s| s.to_string()),
        details,
    };

    let mut line = serde_json::to_string(&entry).context("serialize audit entry")?;
    line.push('\n');

    // Append to file
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&audit_path)
        .with_context(|| format!("open audit log {}", audit_path.display()))?;
    file.write_all(line.as_bytes())
        .with_context(|| format!("write to audit log {}", audit_path.display()))?;

    Ok(())
}

/// Generate a proposal ID from timestamp and slug.
pub fn generate_proposal_id(slug: &str) -> String {
    let timestamp = Utc::now().format("%Y%m%d%H%M%S");
    format!("{}-{}", timestamp, slug)
}

/// Sort graph nodes deterministically (by kind, then id).
pub fn sort_graph_nodes(graph: &mut GraphDoc) {
    graph.nodes.sort_by(|a, b| {
        a.kind.cmp(&b.kind).then_with(|| a.id.cmp(&b.id))
    });
}

/// Lock a file for exclusive write access (blocks until lock is acquired).
/// Returns a guard that releases the lock when dropped.
pub fn lock_graph_file(path: &Path) -> Result<fs::File> {
    use fs2::FileExt;
    
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .with_context(|| format!("open graph file for locking: {}", path.display()))?;
    
    file.lock_exclusive()
        .with_context(|| format!("acquire exclusive lock on {}", path.display()))?;
    
    Ok(file)
}

/// Accept a proposal with concurrency safety (file-level lock).
/// Returns the updated graph hash (SHA256 hex digest).
pub fn accept_proposal_safe(
    proposal_id: &str,
    actor: &str,
    source: Option<&str>,
) -> Result<String> {
    // 1. Find proposal
    let (proposal_path, mut proposal) = find_proposal(proposal_id)?;

    if proposal.status != ProposalStatus::Pending {
        bail!(
            "proposal {} is not pending (status: {:?})",
            proposal.id,
            proposal.status
        );
    }

    // 2. Load and lock target graph
    let graph_path = PathBuf::from(&proposal.target_graph);
    if !graph_path.exists() {
        bail!("target graph {} does not exist", graph_path.display());
    }

    let _lock = lock_graph_file(&graph_path)?;

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
    let updated_content = serde_json::to_string_pretty(&graph)
        .context("serialize updated graph")?;

    // 6. Write back to disk
    std::fs::write(&graph_path, &updated_content)
        .with_context(|| format!("write updated graph to {}", graph_path.display()))?;

    // 7. Compute hash
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(updated_content.as_bytes());
    let hash = format!("{:x}", hasher.finalize());

    // 8. Update proposal status
    proposal.accept();
    proposal.save(&proposal_path)?;

    // 9. Log to audit trail
    audit_log(
        "accept",
        &proposal.id,
        actor,
        source,
        Some(serde_json::json!({"target": proposal.target_graph, "hash": hash})),
    )?;

    Ok(hash)
}
