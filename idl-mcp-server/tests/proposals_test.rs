//! Integration tests for MCP proposal tools.

use serde_json::json;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};
use tempfile::TempDir;

static CWD_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn setup_test_env() -> (TempDir, PathBuf, PathBuf, MutexGuard<'static, ()>) {
    let guard = CWD_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let graph_path = temp.path().join("test.graph.json");
    let orig_dir = std::env::current_dir().unwrap();

    // Create a minimal test graph
    let graph = json!({
        "version": "0.1.0",
        "metadata": {},
        "nodes": [
            {
                "id": "dto-user",
                "kind": "dto",
                "state": "accepted",
                "props": {
                    "dto_props": {
                        "fields": [
                            {"name": "id", "type": "string"},
                            {"name": "name", "type": "string"}
                        ]
                    }
                },
                "source_anchors": [],
                "decision_refs": []
            }
        ],
        "edges": []
    });

    std::fs::write(&graph_path, serde_json::to_string_pretty(&graph).unwrap()).unwrap();

    // Create changes directory
    let changes_dir = temp.path().join("changes");
    std::fs::create_dir(&changes_dir).unwrap();

    // Set current directory to temp (for proposal creation)
    std::env::set_current_dir(temp.path()).unwrap();

    (temp, graph_path, orig_dir, guard)
}

fn teardown_test_env(orig_dir: PathBuf) {
    let _ = std::env::set_current_dir(orig_dir);
}

#[test]
fn test_proposal_workflow_create_get_accept() {
    use idl_proposals::{accept_proposal_safe, find_proposal, DiffOp, Proposal, ProposalStatus};

    let (_temp, graph_path, orig_dir, _guard) = setup_test_env();

    // Create proposal using the library directly
    let diff_ops = vec![DiffOp::ModifyDtoField {
        dto_id: "dto-user".to_string(),
        field_name: "email".to_string(),
        action: idl_proposals::FieldAction::Add,
        field_data: Some(json!({
            "name": "email",
            "type": "string"
        })),
    }];

    let proposal = Proposal::new(
        idl_proposals::generate_proposal_id("add-email"),
        "test-agent".to_string(),
        graph_path.display().to_string(),
        Some("Add email field".to_string()),
        diff_ops,
    );

    let changes_dir = idl_proposals::locate_changes_dir().unwrap();
    let proposal_path = changes_dir.join(format!("{}.proposal.json", proposal.id));
    proposal.save(&proposal_path).unwrap();

    // Log audit
    idl_proposals::audit_log("create", &proposal.id, "test-agent", Some("test"), None).unwrap();

    // Get proposal
    let (_, loaded) = find_proposal(&proposal.id).unwrap();
    assert_eq!(loaded.author, "test-agent");
    assert_eq!(loaded.status, ProposalStatus::Pending);

    // Accept proposal
    let hash = accept_proposal_safe(&proposal.id, "reviewer", Some("test")).unwrap();
    assert!(!hash.is_empty());

    // Verify graph was modified
    let graph_content = std::fs::read_to_string(&graph_path).unwrap();
    assert!(graph_content.contains("email"));

    teardown_test_env(orig_dir);
}

#[test]
fn test_proposal_list_filter() {
    use idl_proposals::{list_proposals, DiffOp, Proposal, ProposalStatus};

    let (_temp, graph_path, orig_dir, _guard) = setup_test_env();

    // Create two proposals
    for i in 1..=2 {
        let diff_ops = vec![DiffOp::ChangeKind {
            node_id: "dto-user".to_string(),
            new_kind: "entity".to_string(),
        }];

        let proposal = Proposal::new(
            idl_proposals::generate_proposal_id(&format!("change{}", i)),
            format!("agent-{}", i),
            graph_path.display().to_string(),
            Some(format!("Change {}", i)),
            diff_ops,
        );

        let changes_dir = idl_proposals::locate_changes_dir().unwrap();
        let proposal_path = changes_dir.join(format!("{}.proposal.json", proposal.id));
        proposal.save(&proposal_path).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // List all
    let proposals = list_proposals(None).unwrap();
    assert_eq!(proposals.len(), 2);

    // Filter by pending
    let pending = list_proposals(Some(ProposalStatus::Pending)).unwrap();
    assert_eq!(pending.len(), 2);

    // Filter by accepted (should be empty)
    let accepted = list_proposals(Some(ProposalStatus::Accepted)).unwrap();
    assert_eq!(accepted.len(), 0);

    teardown_test_env(orig_dir);
}

#[test]
fn test_proposal_reject_no_mutation() {
    use idl_proposals::{audit_log, find_proposal, DiffOp, Proposal};

    let (_temp, graph_path, orig_dir, _guard) = setup_test_env();

    // Read original graph
    let original_content = std::fs::read_to_string(&graph_path).unwrap();

    // Create proposal
    let diff_ops = vec![DiffOp::ModifyDtoField {
        dto_id: "dto-user".to_string(),
        field_name: "age".to_string(),
        action: idl_proposals::FieldAction::Add,
        field_data: Some(json!({
            "name": "age",
            "type": "number"
        })),
    }];

    let proposal = Proposal::new(
        idl_proposals::generate_proposal_id("add-age"),
        "test-agent".to_string(),
        graph_path.display().to_string(),
        Some("Add field".to_string()),
        diff_ops,
    );

    let changes_dir = idl_proposals::locate_changes_dir().unwrap();
    let proposal_path = changes_dir.join(format!("{}.proposal.json", proposal.id));
    proposal.save(&proposal_path).unwrap();

    // Reject proposal
    let (proposal_path, mut proposal) = find_proposal(&proposal.id).unwrap();
    proposal.reject("Not needed".to_string());
    proposal.save(&proposal_path).unwrap();

    audit_log(
        "reject",
        &proposal.id,
        "reviewer",
        Some("test"),
        Some(json!({"reason": "Not needed"})),
    )
    .unwrap();

    // Verify graph was NOT modified
    let current_content = std::fs::read_to_string(&graph_path).unwrap();
    assert_eq!(original_content, current_content);
    assert!(!current_content.contains("age"));

    teardown_test_env(orig_dir);
}

#[test]
fn test_concurrent_accept_safety() {
    use idl_proposals::{
        accept_proposal_safe, generate_proposal_id, locate_changes_dir, DiffOp, Proposal,
    };
    use std::sync::Arc;
    use std::thread;

    let (_temp, graph_path, orig_dir, _guard) = setup_test_env();
    let graph_path_arc = Arc::new(graph_path.clone());

    // Create two proposals targeting the same graph
    let mut proposal_ids = Vec::new();
    for i in 1..=2 {
        let diff_ops = vec![DiffOp::ModifyDtoField {
            dto_id: "dto-user".to_string(),
            field_name: format!("field{}", i),
            action: idl_proposals::FieldAction::Add,
            field_data: Some(json!({
                "name": format!("field{}", i),
                "type": "string"
            })),
        }];

        let proposal = Proposal::new(
            generate_proposal_id(&format!("change{}", i)),
            format!("agent-{}", i),
            graph_path_arc.display().to_string(),
            Some(format!("Change {}", i)),
            diff_ops,
        );

        proposal_ids.push(proposal.id.clone());

        let changes_dir = locate_changes_dir().unwrap();
        let proposal_path = changes_dir.join(format!("{}.proposal.json", proposal.id));
        proposal.save(&proposal_path).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // Try to accept both concurrently
    let handles: Vec<_> = proposal_ids
        .into_iter()
        .map(|id| {
            thread::spawn(move || accept_proposal_safe(&id, "concurrent-reviewer", Some("test")))
        })
        .collect();

    let results: Vec<Result<String, anyhow::Error>> =
        handles.into_iter().map(|h| h.join().unwrap()).collect();

    // Both should succeed (file locking ensures sequential execution)
    assert!(results[0].is_ok());
    assert!(results[1].is_ok());

    // Verify both fields were added
    let graph_content = std::fs::read_to_string(graph_path).unwrap();
    assert!(graph_content.contains("field1"));
    assert!(graph_content.contains("field2"));

    teardown_test_env(orig_dir);
}

#[test]
fn test_audit_log_source_attribution() {
    use idl_proposals::{audit_log, locate_changes_dir};

    let (_temp, _graph_path, orig_dir, _guard) = setup_test_env();

    // Log from MCP
    audit_log("test_action", "test-id", "agent", Some("mcp"), None).unwrap();

    // Log from CLI
    audit_log("test_action2", "test-id2", "user", Some("cli"), None).unwrap();

    // Read audit log
    let changes_dir = locate_changes_dir().unwrap();
    let audit_path = changes_dir.join("audit.jsonl");
    let content = std::fs::read_to_string(audit_path).unwrap();

    // Verify source attribution
    assert!(content.contains(r#""source":"mcp"#));
    assert!(content.contains(r#""source":"cli"#));

    teardown_test_env(orig_dir);
}
