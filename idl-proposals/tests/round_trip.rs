#![allow(clippy::ptr_arg)]
//! Round-trip integration tests for proposal workflows.
//!
//! Tests cover the complete proposal lifecycle:
//! - Happy path: create → accept → verify graph mutation
//! - Reject path: create → reject → verify graph unchanged
//! - Concurrent safety: file-lock prevents torn writes
//! - Rollback: invalid ops fail cleanly without partial application
//! - Schema validation: malformed proposals rejected early

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Barrier, Mutex, MutexGuard, OnceLock};
use std::thread;

use serde_json::json;
use tempfile::TempDir;

use idl_proposals::{
    accept_proposal_safe, audit_log, find_proposal, list_proposals, DiffOp, FieldAction, Proposal,
    ProposalStatus,
};

static CWD_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Helper: Guard to restore current directory on drop (RAII pattern)
struct DirGuard {
    original_dir: PathBuf,
    _guard: MutexGuard<'static, ()>,
}

impl DirGuard {
    fn new() -> anyhow::Result<Self> {
        let guard = CWD_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        Ok(Self {
            original_dir: std::env::current_dir()?,
            _guard: guard,
        })
    }
}

impl Drop for DirGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.original_dir);
    }
}

/// Helper: Create a minimal test graph with DTO nodes.
fn create_test_graph(path: &PathBuf, nodes: serde_json::Value) -> anyhow::Result<()> {
    let graph = json!({
        "version": "0.1.9",
        "metadata": {
            "corpus": "test",
            "description": "Test graph for proposal round-trip"
        },
        "nodes": nodes,
        "edges": [],
        "extensions": {
            "dto": {
                "definitions": []
            }
        }
    });
    fs::write(path, serde_json::to_string_pretty(&graph)?)?;
    Ok(())
}

/// Helper: Create and save a proposal.
fn create_and_save_proposal(
    changes_dir: &PathBuf,
    id: &str,
    target_graph: &str,
    diff_ops: Vec<DiffOp>,
    temp_path: &std::path::Path,
) -> anyhow::Result<PathBuf> {
    let proposal = Proposal::new(
        id.to_string(),
        "test-agent".to_string(),
        target_graph.to_string(),
        Some("Integration test proposal".to_string()),
        diff_ops,
    );

    let proposal_path = changes_dir.join(format!("{}.proposal.json", id));
    proposal.save(&proposal_path)?;

    // Log creation to audit trail (needs to be in correct dir)
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_path)?;
    audit_log(
        "create",
        &proposal.id,
        "test-agent",
        Some("integration-test"),
        None,
    )?;
    std::env::set_current_dir(original_dir)?;

    Ok(proposal_path)
}

/// Helper: Read audit.jsonl and verify entries.
fn verify_audit_entries(changes_dir: &PathBuf, expected_actions: &[&str]) -> anyhow::Result<()> {
    let audit_path = changes_dir.join("audit.jsonl");
    let audit_content = fs::read_to_string(&audit_path)?;

    let lines: Vec<&str> = audit_content.lines().collect();
    assert_eq!(
        lines.len(),
        expected_actions.len(),
        "Expected {} audit entries, found {}",
        expected_actions.len(),
        lines.len()
    );

    for (line, expected_action) in lines.iter().zip(expected_actions.iter()) {
        let entry: serde_json::Value = serde_json::from_str(line)?;
        assert_eq!(entry["action"].as_str().unwrap(), *expected_action);
        assert!(entry["timestamp"].is_string());
        assert!(entry["proposal_id"].is_string());
        assert!(entry["author"].is_string());
    }

    Ok(())
}

/// TEST 1: Happy path accept
/// Create proposal with 3 ops (add_dto, modify_dto_field, change_kind).
/// Accept it. Verify graph mutated correctly. Verify audit trail.
#[test]
fn test_happy_path_accept() -> anyhow::Result<()> {
    let _dir_guard = DirGuard::new()?; // Ensure we restore directory on exit

    let temp = TempDir::new()?;
    let graph_path = temp.path().join("test.graph.json");
    let changes_dir = temp.path().join("changes");
    fs::create_dir_all(&changes_dir)?;

    // Create initial graph with one DTO
    create_test_graph(
        &graph_path,
        json!([
            {
                "id": "dto:Existing",
                "kind": "dto",
                "state": "accepted",
                "props": {
                    "dto_props": {
                        "kind": "object",
                        "fields": [
                            {
                                "name": "id",
                                "type": "string",
                                "required": true
                            }
                        ]
                    }
                }
            }
        ]),
    )?;

    // Create proposal with 3 operations
    let diff_ops = vec![
        // Op 1: Add a new DTO
        DiffOp::AddDto {
            dto: serde_json::from_value(json!({
                "id": "dto:NewUser",
                "kind": "dto",
                "state": "accepted",
                "props": {
                    "dto_props": {
                        "kind": "object",
                        "fields": [
                            {
                                "name": "username",
                                "type": "string",
                                "required": true
                            }
                        ]
                    }
                }
            }))?,
        },
        // Op 2: Add a field to existing DTO
        DiffOp::ModifyDtoField {
            dto_id: "dto:Existing".to_string(),
            field_name: "email".to_string(),
            action: FieldAction::Add,
            field_data: Some(json!({
                "name": "email",
                "type": "string",
                "required": false
            })),
        },
        // Op 3: Change kind of existing DTO
        DiffOp::ChangeKind {
            node_id: "dto:Existing".to_string(),
            new_kind: "entity".to_string(),
        },
    ];

    let proposal_id = "20260501120000-multi-op";
    create_and_save_proposal(
        &changes_dir,
        proposal_id,
        graph_path.to_str().unwrap(),
        diff_ops,
        temp.path(),
    )?;

    // Accept proposal
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp.path())?;
    let hash = accept_proposal_safe(proposal_id, "test-reviewer", Some("integration-test"))?;
    std::env::set_current_dir(original_dir)?;

    // Verify hash is non-empty
    assert!(!hash.is_empty());
    assert_eq!(hash.len(), 64); // SHA256 hex is 64 chars

    // Verify graph was mutated
    let updated_graph: serde_json::Value = serde_json::from_str(&fs::read_to_string(&graph_path)?)?;
    let nodes = updated_graph["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 2, "Should have 2 nodes after adding NewUser");

    // Verify new DTO was added
    let new_user = nodes
        .iter()
        .find(|n| n["id"] == "dto:NewUser")
        .expect("NewUser not found");
    assert_eq!(new_user["kind"], "dto");

    // Verify field was added to existing DTO
    let existing = nodes
        .iter()
        .find(|n| n["id"] == "dto:Existing")
        .expect("Existing not found");
    assert_eq!(existing["kind"], "entity"); // Kind changed
    let fields = existing["props"]["dto_props"]["fields"].as_array().unwrap();
    assert_eq!(fields.len(), 2, "Should have 2 fields after adding email");
    assert!(fields.iter().any(|f| f["name"] == "email"));

    // Verify proposal status is accepted
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp.path())?;
    let (_, updated_proposal) = find_proposal(proposal_id)?;
    std::env::set_current_dir(original_dir)?;

    assert_eq!(updated_proposal.status, ProposalStatus::Accepted);
    assert!(updated_proposal.updated_at.is_some());

    // Verify audit trail has 2 entries: create + accept
    verify_audit_entries(&changes_dir, &["create", "accept"])?;

    Ok(())
}

/// TEST 2: Reject path
/// Create proposal, reject with reason. Verify graph UNCHANGED. Verify audit.
#[test]
fn test_reject_path() -> anyhow::Result<()> {
    let _dir_guard = DirGuard::new()?; // Ensure we restore directory on exit

    let temp = TempDir::new()?;
    let graph_path = temp.path().join("test.graph.json");
    let changes_dir = temp.path().join("changes");
    fs::create_dir_all(&changes_dir)?;

    // Create initial graph
    create_test_graph(
        &graph_path,
        json!([
            {
                "id": "dto:Original",
                "kind": "dto",
                "state": "accepted",
                "props": {
                    "dto_props": {
                        "kind": "object",
                        "fields": []
                    }
                }
            }
        ]),
    )?;

    // Compute original hash
    let original_content = fs::read_to_string(&graph_path)?;

    // Create proposal
    let diff_ops = vec![DiffOp::AddDto {
        dto: serde_json::from_value(json!({
            "id": "dto:Unwanted",
            "kind": "dto",
            "state": "accepted",
            "props": {
                "dto_props": {
                    "kind": "object",
                    "fields": []
                }
            }
        }))?,
    }];

    let proposal_id = "20260501130000-unwanted";
    let proposal_path = create_and_save_proposal(
        &changes_dir,
        proposal_id,
        graph_path.to_str().unwrap(),
        diff_ops,
        temp.path(),
    )?;

    // Reject proposal
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp.path())?;
    let mut proposal = Proposal::load(&proposal_path)?;
    proposal.reject("Does not align with architecture".to_string());
    proposal.save(&proposal_path)?;
    audit_log(
        "reject",
        &proposal.id,
        "test-reviewer",
        Some("integration-test"),
        Some(json!({"reason": "Does not align with architecture"})),
    )?;
    std::env::set_current_dir(original_dir)?;

    // Verify graph UNCHANGED
    let current_content = fs::read_to_string(&graph_path)?;
    assert_eq!(
        original_content, current_content,
        "Graph should be unchanged after rejection"
    );

    let graph: serde_json::Value = serde_json::from_str(&current_content)?;
    let nodes = graph["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1, "Should still have only 1 node");
    assert_eq!(nodes[0]["id"], "dto:Original");

    // Verify proposal status is rejected
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp.path())?;
    let (_, updated_proposal) = find_proposal(proposal_id)?;
    std::env::set_current_dir(original_dir)?;

    assert_eq!(updated_proposal.status, ProposalStatus::Rejected);
    assert_eq!(
        updated_proposal.rejection_reason.as_ref().unwrap(),
        "Does not align with architecture"
    );
    assert!(updated_proposal.updated_at.is_some());

    // Verify audit trail has 2 entries: create + reject
    verify_audit_entries(&changes_dir, &["create", "reject"])?;

    Ok(())
}

/// TEST 3: Concurrent accept (file-lock test)
/// Spawn 2 threads each accepting a different proposal targeting same graph.
/// Verify only one mutates at a time, final state consistent, no torn writes.
#[test]
fn test_concurrent_accept_file_lock() -> anyhow::Result<()> {
    let _dir_guard = DirGuard::new()?; // Ensure we restore directory on exit

    let temp = TempDir::new()?;
    let graph_path = temp.path().join("test.graph.json");
    let changes_dir = temp.path().join("changes");
    fs::create_dir_all(&changes_dir)?;

    // Create initial empty graph
    create_test_graph(&graph_path, json!([]))?;

    // Create two proposals that add different DTOs
    let proposal_id_1 = "20260501140000-concurrent-1";
    let proposal_id_2 = "20260501140001-concurrent-2";

    create_and_save_proposal(
        &changes_dir,
        proposal_id_1,
        graph_path.to_str().unwrap(),
        vec![DiffOp::AddDto {
            dto: serde_json::from_value(json!({
                "id": "dto:Concurrent1",
                "kind": "dto",
                "state": "accepted",
                "props": {
                    "dto_props": {
                        "kind": "object",
                        "fields": []
                    }
                }
            }))?,
        }],
        temp.path(),
    )?;

    create_and_save_proposal(
        &changes_dir,
        proposal_id_2,
        graph_path.to_str().unwrap(),
        vec![DiffOp::AddDto {
            dto: serde_json::from_value(json!({
                "id": "dto:Concurrent2",
                "kind": "dto",
                "state": "accepted",
                "props": {
                    "dto_props": {
                        "kind": "object",
                        "fields": []
                    }
                }
            }))?,
        }],
        temp.path(),
    )?;

    // Synchronize threads
    let barrier = Arc::new(Barrier::new(2));
    let temp_path = Arc::new(temp.path().to_path_buf());

    let barrier1 = Arc::clone(&barrier);
    let barrier2 = Arc::clone(&barrier);
    let temp_path1 = Arc::clone(&temp_path);
    let temp_path2 = Arc::clone(&temp_path);

    // Spawn two threads that accept proposals simultaneously
    // Both threads will work in the same temp directory to avoid directory race conditions
    let handle1 = thread::spawn(move || {
        std::env::set_current_dir(&*temp_path1).unwrap();
        barrier1.wait(); // Synchronize start
        accept_proposal_safe(proposal_id_1, "test-reviewer-1", Some("integration-test"))
    });

    let handle2 = thread::spawn(move || {
        std::env::set_current_dir(&*temp_path2).unwrap();
        barrier2.wait(); // Synchronize start
        accept_proposal_safe(proposal_id_2, "test-reviewer-2", Some("integration-test"))
    });

    // Wait for both threads
    let result1 = handle1.join().expect("thread 1 panicked");
    let result2 = handle2.join().expect("thread 2 panicked");

    // Both should succeed (file lock prevents corruption)
    assert!(result1.is_ok(), "Proposal 1 should succeed: {:?}", result1);
    assert!(result2.is_ok(), "Proposal 2 should succeed: {:?}", result2);

    // Verify final graph state
    let final_graph: serde_json::Value = serde_json::from_str(&fs::read_to_string(&graph_path)?)?;
    let nodes = final_graph["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 2, "Should have both DTOs added");

    let ids: Vec<&str> = nodes.iter().map(|n| n["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&"dto:Concurrent1"));
    assert!(ids.contains(&"dto:Concurrent2"));

    // Verify graph is valid JSON (no torn writes)
    let graph_content = fs::read_to_string(&graph_path)?;
    let _parsed: serde_json::Value = serde_json::from_str(&graph_content)
        .expect("Graph should be valid JSON after concurrent writes");

    // Verify both proposals are marked accepted
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp.path())?;
    let proposals = list_proposals(None)?;
    std::env::set_current_dir(original_dir)?;

    let accepted_count = proposals
        .iter()
        .filter(|(_, p)| p.status == ProposalStatus::Accepted)
        .count();
    assert_eq!(accepted_count, 2, "Both proposals should be accepted");

    Ok(())
}

/// TEST 4: Failed apply rollback
/// Create proposal with invalid op (remove_dto referencing nonexistent DTO).
/// Verify accept fails cleanly, graph UNCHANGED, audit shows attempt + failure.
#[test]
fn test_failed_apply_rollback() -> anyhow::Result<()> {
    let _dir_guard = DirGuard::new()?; // Ensure we restore directory on exit

    let temp = TempDir::new()?;
    let graph_path = temp.path().join("test.graph.json");
    let changes_dir = temp.path().join("changes");
    fs::create_dir_all(&changes_dir)?;

    // Create initial graph
    create_test_graph(
        &graph_path,
        json!([
            {
                "id": "dto:Existing",
                "kind": "dto",
                "state": "accepted",
                "props": {
                    "dto_props": {
                        "kind": "object",
                        "fields": []
                    }
                }
            }
        ]),
    )?;

    // Compute original content hash
    let original_content = fs::read_to_string(&graph_path)?;

    // Create proposal with invalid operation (remove nonexistent node)
    let diff_ops = vec![
        DiffOp::AddDto {
            dto: serde_json::from_value(json!({
                "id": "dto:WillNotApply",
                "kind": "dto",
                "state": "accepted",
                "props": {
                    "dto_props": {
                        "kind": "object",
                        "fields": []
                    }
                }
            }))?,
        },
        DiffOp::RemoveDto {
            node_id: "dto:NonExistent".to_string(),
        },
    ];

    let proposal_id = "20260501150000-invalid";
    create_and_save_proposal(
        &changes_dir,
        proposal_id,
        graph_path.to_str().unwrap(),
        diff_ops,
        temp.path(),
    )?;

    // Attempt to accept proposal (should fail)
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp.path())?;
    let result = accept_proposal_safe(proposal_id, "test-reviewer", Some("integration-test"));
    std::env::set_current_dir(original_dir)?;

    assert!(result.is_err(), "Accept should fail for invalid operation");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("not found")
            || err_msg.contains("NonExistent")
            || err_msg.contains("apply diff ops"),
        "Error should mention missing node or apply failure: {}",
        err_msg
    );

    // Verify graph is UNCHANGED (rollback worked)
    let current_content = fs::read_to_string(&graph_path)?;
    assert_eq!(
        original_content, current_content,
        "Graph should be unchanged after failed apply"
    );

    let graph: serde_json::Value = serde_json::from_str(&current_content)?;
    let nodes = graph["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1, "Should still have only 1 node");
    assert_eq!(nodes[0]["id"], "dto:Existing");

    // Verify proposal is still pending (not marked accepted)
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp.path())?;
    let (_, proposal) = find_proposal(proposal_id)?;
    std::env::set_current_dir(original_dir)?;

    assert_eq!(
        proposal.status,
        ProposalStatus::Pending,
        "Proposal should remain pending after failed apply"
    );

    // Verify audit trail has only create entry (no accept since it failed)
    verify_audit_entries(&changes_dir, &["create"])?;

    Ok(())
}

/// TEST 5: Schema validation
/// Try to create proposal with malformed op JSON. Verify rejection with clear error.
#[test]
fn test_schema_validation() -> anyhow::Result<()> {
    let _dir_guard = DirGuard::new()?; // Ensure we restore directory on exit

    let temp = TempDir::new()?;
    let changes_dir = temp.path().join("changes");
    fs::create_dir_all(&changes_dir)?;

    // Create a malformed proposal JSON (missing required fields)
    let malformed_json = json!({
        "version": "0.1.0",
        "id": "20260501160000-malformed",
        "author": "test-agent",
        // Missing target_graph
        "diff_ops": [],
        "status": "pending",
        "created_at": "2026-05-01T16:00:00Z"
    });

    let proposal_path = changes_dir.join("20260501160000-malformed.proposal.json");
    fs::write(
        &proposal_path,
        serde_json::to_string_pretty(&malformed_json)?,
    )?;

    // Try to load the malformed proposal
    let result = Proposal::load(&proposal_path);
    assert!(result.is_err(), "Loading malformed proposal should fail");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("missing field")
            || err_msg.contains("target_graph")
            || err_msg.contains("parse proposal"),
        "Error should mention missing field: {}",
        err_msg
    );

    // Create proposal with invalid diff op (missing required field in AddDto)
    let invalid_op_json = json!({
        "op": "add_dto"
        // Missing "dto" field
    });

    let result = serde_json::from_value::<DiffOp>(invalid_op_json);
    assert!(result.is_err(), "Parsing invalid diff op should fail");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("missing field") || err_msg.contains("dto"),
        "Error should mention missing dto field: {}",
        err_msg
    );

    // Create proposal with wrong version
    let wrong_version_json = json!({
        "version": "99.99.99",
        "id": "20260501160001-wrong-version",
        "author": "test-agent",
        "target_graph": "test.graph.json",
        "diff_ops": [],
        "status": "pending",
        "created_at": "2026-05-01T16:00:00Z"
    });

    let proposal_path2 = changes_dir.join("20260501160001-wrong-version.proposal.json");
    fs::write(
        &proposal_path2,
        serde_json::to_string_pretty(&wrong_version_json)?,
    )?;

    let result = Proposal::load(&proposal_path2);
    assert!(
        result.is_err(),
        "Loading wrong-version proposal should fail"
    );

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("unsupported proposal version") || err_msg.contains("99.99.99"),
        "Error should mention unsupported version: {}",
        err_msg
    );

    Ok(())
}
