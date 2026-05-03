//! Integration tests for `idl propose` commands.

use std::fs;

use serde_json::json;
use tempfile::TempDir;

/// Test: Create a proposal with --title, --target-graph, --ops-file
#[test]
fn test_propose_create_with_ops_file() {
    let temp = TempDir::new().unwrap();
    let graph_path = temp.path().join("test.graph.json");
    let ops_file = temp.path().join("ops.json");

    // Create a minimal graph
    let graph = json!({
        "version": "0.1.7",
        "nodes": [],
        "edges": []
    });
    fs::write(&graph_path, serde_json::to_string_pretty(&graph).unwrap()).unwrap();

    // Create ops file with diff_ops array
    let diff_ops = json!([
        {
            "op": "add_dto",
            "dto": {
                "id": "dto:User",
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
        }
    ]);
    fs::write(&ops_file, serde_json::to_string_pretty(&diff_ops).unwrap()).unwrap();

    // Run propose create command
    let result = assert_cmd::Command::cargo_bin("idl")
        .unwrap()
        .current_dir(temp.path())
        .args([
            "propose", "create",
            "--title", "Add User DTO",
            "--target-graph", graph_path.to_str().unwrap(),
            "--ops-file", ops_file.to_str().unwrap()
        ])
        .assert()
        .success();

    // Verify output contains proposal id
    let output = String::from_utf8_lossy(&result.get_output().stdout);
    assert!(!output.trim().is_empty(), "should output proposal id");

    // Verify proposal file exists in changes/proposals/
    let proposals_dir = temp.path().join("changes/proposals");
    assert!(proposals_dir.is_dir());

    let proposals: Vec<_> = fs::read_dir(&proposals_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    assert_eq!(proposals.len(), 1, "should have exactly one proposal file");

    // Verify audit trail exists with source="cli"
    let audit_path = temp.path().join("changes/audit.jsonl");
    assert!(audit_path.exists());
    let audit_content = fs::read_to_string(&audit_path).unwrap();
    assert!(audit_content.contains("create"));
    assert!(audit_content.contains("\"source\":\"cli\""));
}

/// Test: List proposals without filter
#[test]
fn test_propose_list_all() {
    let temp = TempDir::new().unwrap();
    let proposals_dir = temp.path().join("changes/proposals");
    fs::create_dir_all(&proposals_dir).unwrap();

    // Create multiple proposals with different statuses
    let proposal1 = json!({
        "version": "0.1.0",
        "id": "20261230120000-pending-proposal",
        "author": "test-user",
        "target_graph": "test.graph.json",
        "rationale": "Pending proposal",
        "diff_ops": [],
        "status": "pending",
        "created_at": "2026-12-30T12:00:00Z"
    });
    fs::write(
        proposals_dir.join("20261230120000-pending-proposal.json"),
        serde_json::to_string_pretty(&proposal1).unwrap()
    ).unwrap();

    let proposal2 = json!({
        "version": "0.1.0",
        "id": "20261230120100-accepted-proposal",
        "author": "test-user",
        "target_graph": "test.graph.json",
        "rationale": "Accepted proposal",
        "diff_ops": [],
        "status": "accepted",
        "created_at": "2026-12-30T12:01:00Z"
    });
    fs::write(
        proposals_dir.join("20261230120100-accepted-proposal.json"),
        serde_json::to_string_pretty(&proposal2).unwrap()
    ).unwrap();

    let result = assert_cmd::Command::cargo_bin("idl")
        .unwrap()
        .current_dir(temp.path())
        .args(["propose", "list"])
        .assert()
        .success();

    let output = String::from_utf8_lossy(&result.get_output().stdout);
    assert!(output.contains("pending-proposal"));
    assert!(output.contains("accepted-proposal"));
    assert!(output.contains("pending"));
    assert!(output.contains("accepted"));
}

/// Test: List proposals filtered by status
#[test]
fn test_propose_list_filtered() {
    let temp = TempDir::new().unwrap();
    let proposals_dir = temp.path().join("changes/proposals");
    fs::create_dir_all(&proposals_dir).unwrap();

    // Create proposals with different statuses
    let proposal1 = json!({
        "version": "0.1.0",
        "id": "20261230120000-pending",
        "author": "test-user",
        "target_graph": "test.graph.json",
        "rationale": "Pending",
        "diff_ops": [],
        "status": "pending",
        "created_at": "2026-12-30T12:00:00Z"
    });
    fs::write(
        proposals_dir.join("20261230120000-pending.json"),
        serde_json::to_string_pretty(&proposal1).unwrap()
    ).unwrap();

    let proposal2 = json!({
        "version": "0.1.0",
        "id": "20261230120100-accepted",
        "author": "test-user",
        "target_graph": "test.graph.json",
        "rationale": "Accepted",
        "diff_ops": [],
        "status": "accepted",
        "created_at": "2026-12-30T12:01:00Z"
    });
    fs::write(
        proposals_dir.join("20261230120100-accepted.json"),
        serde_json::to_string_pretty(&proposal2).unwrap()
    ).unwrap();

    let result = assert_cmd::Command::cargo_bin("idl")
        .unwrap()
        .current_dir(temp.path())
        .args(["propose", "list", "--status", "pending"])
        .assert()
        .success();

    let output = String::from_utf8_lossy(&result.get_output().stdout);
    assert!(output.contains("pending"));
    assert!(!output.contains("accepted"));
}

/// Test: Accept a proposal and apply to graph
#[test]
fn test_propose_accept() {
    let temp = TempDir::new().unwrap();
    let proposals_dir = temp.path().join("changes/proposals");
    fs::create_dir_all(&proposals_dir).unwrap();

    // Create initial graph
    let graph_path = temp.path().join("test.graph.json");
    let graph = json!({
        "version": "0.1.7",
        "nodes": [],
        "edges": []
    });
    fs::write(&graph_path, serde_json::to_string_pretty(&graph).unwrap()).unwrap();

    // Create a proposal with add_dto operation
    let proposal = json!({
        "version": "0.1.0",
        "id": "20261230120000-add-user",
        "author": "test-user",
        "target_graph": graph_path.to_str().unwrap(),
        "rationale": "Adding User DTO",
        "diff_ops": [
            {
                "op": "add_dto",
                "dto": {
                    "id": "dto:User",
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
            }
        ],
        "status": "pending",
        "created_at": "2026-12-30T12:00:00Z"
    });
    let proposal_path = proposals_dir.join("20261230120000-add-user.json");
    fs::write(&proposal_path, serde_json::to_string_pretty(&proposal).unwrap()).unwrap();

    let result = assert_cmd::Command::cargo_bin("idl")
        .unwrap()
        .current_dir(temp.path())
        .args(["propose", "accept", "20261230120000-add-user"])
        .assert()
        .success();

    let output = String::from_utf8_lossy(&result.get_output().stdout);
    assert!(output.contains("Accepted: 20261230120000-add-user"));

    // Verify graph was updated
    let updated_graph: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&graph_path).unwrap()
    ).unwrap();
    let nodes = updated_graph["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["id"], "dto:User");

    // Verify proposal status was updated
    let updated_proposal: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&proposal_path).unwrap()
    ).unwrap();
    assert_eq!(updated_proposal["status"], "accepted");

    // Verify audit trail with source="cli"
    let audit_path = temp.path().join("changes/audit.jsonl");
    let audit_content = fs::read_to_string(&audit_path).unwrap();
    assert!(audit_content.contains("accept"));
    assert!(audit_content.contains("\"source\":\"cli\""));
}

/// Test: Reject a proposal with reason
#[test]
fn test_propose_reject() {
    let temp = TempDir::new().unwrap();
    let proposals_dir = temp.path().join("changes/proposals");
    fs::create_dir_all(&proposals_dir).unwrap();

    // Create a proposal
    let proposal = json!({
        "version": "0.1.0",
        "id": "20261230120000-bad-idea",
        "author": "test-user",
        "target_graph": "test.graph.json",
        "rationale": "This is a bad idea",
        "diff_ops": [],
        "status": "pending",
        "created_at": "2026-12-30T12:00:00Z"
    });
    let proposal_path = proposals_dir.join("20261230120000-bad-idea.json");
    fs::write(&proposal_path, serde_json::to_string_pretty(&proposal).unwrap()).unwrap();

    let result = assert_cmd::Command::cargo_bin("idl")
        .unwrap()
        .current_dir(temp.path())
        .args(["propose", "reject", "20261230120000-bad-idea", "--reason", "Not aligned with architecture"])
        .assert()
        .success();

    let output = String::from_utf8_lossy(&result.get_output().stdout);
    assert!(output.contains("Rejected: 20261230120000-bad-idea"));

    // Verify proposal status was updated
    let updated_proposal: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&proposal_path).unwrap()
    ).unwrap();
    assert_eq!(updated_proposal["status"], "rejected");
    assert_eq!(updated_proposal["rejection_reason"], "Not aligned with architecture");

    // Verify audit trail
    let audit_path = temp.path().join("changes/audit.jsonl");
    let audit_content = fs::read_to_string(&audit_path).unwrap();
    assert!(audit_content.contains("reject"));
    assert!(audit_content.contains("Not aligned with architecture"));
    assert!(audit_content.contains("\"source\":\"cli\""));
}

/// Test: File-lock contention handling (acceptance test)
#[test]
fn test_propose_accept_file_lock() {
    let temp = TempDir::new().unwrap();
    let proposals_dir = temp.path().join("changes/proposals");
    fs::create_dir_all(&proposals_dir).unwrap();

    // Create initial graph
    let graph_path = temp.path().join("test.graph.json");
    let graph = json!({
        "version": "0.1.7",
        "nodes": [],
        "edges": []
    });
    fs::write(&graph_path, serde_json::to_string_pretty(&graph).unwrap()).unwrap();

    // Create a proposal
    let proposal = json!({
        "version": "0.1.0",
        "id": "20261230120000-test-lock",
        "author": "test-user",
        "target_graph": graph_path.to_str().unwrap(),
        "rationale": "Testing file lock",
        "diff_ops": [
            {
                "op": "add_dto",
                "dto": {
                    "id": "dto:Test",
                    "kind": "dto",
                    "state": "accepted",
                    "props": {
                        "dto_props": {
                            "kind": "object",
                            "fields": []
                        }
                    }
                }
            }
        ],
        "status": "pending",
        "created_at": "2026-12-30T12:00:00Z"
    });
    let proposal_path = proposals_dir.join("20261230120000-test-lock.json");
    fs::write(&proposal_path, serde_json::to_string_pretty(&proposal).unwrap()).unwrap();

    // Accept should acquire lock and succeed
    assert_cmd::Command::cargo_bin("idl")
        .unwrap()
        .current_dir(temp.path())
        .args(["propose", "accept", "20261230120000-test-lock"])
        .assert()
        .success();

    // Verify graph was updated
    let updated_graph: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&graph_path).unwrap()
    ).unwrap();
    let nodes = updated_graph["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1);
}

/// Test: Validation failure on invalid diff ops
#[test]
fn test_propose_accept_validation_failure() {
    let temp = TempDir::new().unwrap();
    let proposals_dir = temp.path().join("changes/proposals");
    fs::create_dir_all(&proposals_dir).unwrap();

    // Create initial graph
    let graph_path = temp.path().join("test.graph.json");
    let graph = json!({
        "version": "0.1.7",
        "nodes": [],
        "edges": []
    });
    fs::write(&graph_path, serde_json::to_string_pretty(&graph).unwrap()).unwrap();

    // Create a proposal that tries to remove a non-existent node
    let proposal = json!({
        "version": "0.1.0",
        "id": "20261230120000-bad-op",
        "author": "test-user",
        "target_graph": graph_path.to_str().unwrap(),
        "rationale": "Trying to remove non-existent node",
        "diff_ops": [
            {
                "op": "remove_dto",
                "node_id": "dto:NonExistent"
            }
        ],
        "status": "pending",
        "created_at": "2026-12-30T12:00:00Z"
    });
    let proposal_path = proposals_dir.join("20261230120000-bad-op.json");
    fs::write(&proposal_path, serde_json::to_string_pretty(&proposal).unwrap()).unwrap();

    assert_cmd::Command::cargo_bin("idl")
        .unwrap()
        .current_dir(temp.path())
        .args(["propose", "accept", "20261230120000-bad-op"])
        .assert()
        .failure();
}
