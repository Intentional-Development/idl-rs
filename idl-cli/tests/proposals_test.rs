//! Integration tests for `idl propose` MVP.

use std::fs;
use std::path::PathBuf;

use serde_json::json;
use tempfile::TempDir;

/// Test: Create a proposal and verify file structure
#[test]
fn test_propose_creates_proposal_file() {
    let temp = TempDir::new().unwrap();
    let graph_path = temp.path().join("test.graph.json");
    let change_spec_path = temp.path().join("change.json");

    // Create a minimal graph
    let graph = json!({
        "version": "0.1.7",
        "nodes": [],
        "edges": []
    });
    fs::write(&graph_path, serde_json::to_string_pretty(&graph).unwrap()).unwrap();

    // Create a change spec
    let change_spec = json!({
        "author": "test-user",
        "slug": "add-user-dto",
        "rationale": "Adding User DTO for authentication",
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
        ]
    });
    fs::write(&change_spec_path, serde_json::to_string_pretty(&change_spec).unwrap()).unwrap();

    // Run propose command
    std::env::set_current_dir(temp.path()).unwrap();
    let result = assert_cmd::Command::cargo_bin("idl")
        .unwrap()
        .args(["propose", graph_path.to_str().unwrap(), change_spec_path.to_str().unwrap()])
        .assert()
        .success();

    // Verify output mentions proposal creation
    let output = String::from_utf8_lossy(&result.get_output().stdout);
    assert!(output.contains("proposal created"));

    // Verify proposal file exists
    let changes_dir = temp.path().join("changes");
    assert!(changes_dir.is_dir());

    let proposals: Vec<_> = fs::read_dir(&changes_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    assert_eq!(proposals.len(), 1, "should have exactly one proposal file");

    // Verify audit trail exists
    let audit_path = changes_dir.join("audit.jsonl");
    assert!(audit_path.exists());
    let audit_content = fs::read_to_string(&audit_path).unwrap();
    assert!(audit_content.contains("propose"));
}

/// Test: List proposals
#[test]
fn test_proposals_list() {
    let temp = TempDir::new().unwrap();
    let changes_dir = temp.path().join("changes");
    fs::create_dir_all(&changes_dir).unwrap();

    // Create a proposal file
    let proposal = json!({
        "version": "0.1.0",
        "id": "20261230120000-test",
        "author": "test-user",
        "target_graph": "test.graph.json",
        "rationale": "Test proposal",
        "diff_ops": [],
        "status": "pending",
        "created_at": "2026-12-30T12:00:00Z"
    });
    fs::write(
        changes_dir.join("20261230120000-test.proposal.json"),
        serde_json::to_string_pretty(&proposal).unwrap()
    ).unwrap();

    std::env::set_current_dir(temp.path()).unwrap();
    let result = assert_cmd::Command::cargo_bin("idl")
        .unwrap()
        .args(["proposals", "list"])
        .assert()
        .success();

    let output = String::from_utf8_lossy(&result.get_output().stdout);
    assert!(output.contains("20261230120000-test"));
    assert!(output.contains("pending"));
    assert!(output.contains("test-user"));
}

/// Test: Accept a proposal and apply to graph
#[test]
fn test_proposals_accept() {
    let temp = TempDir::new().unwrap();
    let changes_dir = temp.path().join("changes");
    fs::create_dir_all(&changes_dir).unwrap();

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
    let proposal_path = changes_dir.join("20261230120000-add-user.proposal.json");
    fs::write(&proposal_path, serde_json::to_string_pretty(&proposal).unwrap()).unwrap();

    std::env::set_current_dir(temp.path()).unwrap();
    assert_cmd::Command::cargo_bin("idl")
        .unwrap()
        .args(["proposals", "accept", "20261230120000-add-user"])
        .assert()
        .success();

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

    // Verify audit trail
    let audit_path = changes_dir.join("audit.jsonl");
    let audit_content = fs::read_to_string(&audit_path).unwrap();
    assert!(audit_content.contains("accept"));
}

/// Test: Reject a proposal
#[test]
fn test_proposals_reject() {
    let temp = TempDir::new().unwrap();
    let changes_dir = temp.path().join("changes");
    fs::create_dir_all(&changes_dir).unwrap();

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
    let proposal_path = changes_dir.join("20261230120000-bad-idea.proposal.json");
    fs::write(&proposal_path, serde_json::to_string_pretty(&proposal).unwrap()).unwrap();

    std::env::set_current_dir(temp.path()).unwrap();
    assert_cmd::Command::cargo_bin("idl")
        .unwrap()
        .args(["proposals", "reject", "20261230120000-bad-idea", "--reason", "Not aligned with architecture"])
        .assert()
        .success();

    // Verify proposal status was updated
    let updated_proposal: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&proposal_path).unwrap()
    ).unwrap();
    assert_eq!(updated_proposal["status"], "rejected");
    assert_eq!(updated_proposal["rejection_reason"], "Not aligned with architecture");

    // Verify audit trail
    let audit_path = changes_dir.join("audit.jsonl");
    let audit_content = fs::read_to_string(&audit_path).unwrap();
    assert!(audit_content.contains("reject"));
    assert!(audit_content.contains("Not aligned with architecture"));
}

/// Test: Validation failure on invalid diff ops
#[test]
fn test_proposals_accept_validation_failure() {
    let temp = TempDir::new().unwrap();
    let changes_dir = temp.path().join("changes");
    fs::create_dir_all(&changes_dir).unwrap();

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
    let proposal_path = changes_dir.join("20261230120000-bad-op.proposal.json");
    fs::write(&proposal_path, serde_json::to_string_pretty(&proposal).unwrap()).unwrap();

    std::env::set_current_dir(temp.path()).unwrap();
    assert_cmd::Command::cargo_bin("idl")
        .unwrap()
        .args(["proposals", "accept", "20261230120000-bad-op"])
        .assert()
        .failure();
}

/// Test: Modify DTO field operation
#[test]
fn test_modify_dto_field_operation() {
    let temp = TempDir::new().unwrap();
    let changes_dir = temp.path().join("changes");
    fs::create_dir_all(&changes_dir).unwrap();

    // Create initial graph with a DTO
    let graph_path = temp.path().join("test.graph.json");
    let graph = json!({
        "version": "0.1.7",
        "nodes": [
            {
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
        ],
        "edges": []
    });
    fs::write(&graph_path, serde_json::to_string_pretty(&graph).unwrap()).unwrap();

    // Create a proposal that adds a field
    let proposal = json!({
        "version": "0.1.0",
        "id": "20261230120000-add-field",
        "author": "test-user",
        "target_graph": graph_path.to_str().unwrap(),
        "rationale": "Adding email field to User",
        "diff_ops": [
            {
                "op": "modify_dto_field",
                "dto_id": "dto:User",
                "field_name": "email",
                "action": "add",
                "field_data": {
                    "name": "email",
                    "type": "string",
                    "required": true
                }
            }
        ],
        "status": "pending",
        "created_at": "2026-12-30T12:00:00Z"
    });
    let proposal_path = changes_dir.join("20261230120000-add-field.proposal.json");
    fs::write(&proposal_path, serde_json::to_string_pretty(&proposal).unwrap()).unwrap();

    std::env::set_current_dir(temp.path()).unwrap();
    assert_cmd::Command::cargo_bin("idl")
        .unwrap()
        .args(["proposals", "accept", "20261230120000-add-field"])
        .assert()
        .success();

    // Verify field was added
    let updated_graph: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&graph_path).unwrap()
    ).unwrap();
    let fields = updated_graph["nodes"][0]["props"]["dto_props"]["fields"]
        .as_array()
        .unwrap();
    assert_eq!(fields.len(), 2);
    assert!(fields.iter().any(|f| f["name"] == "email"));
}
