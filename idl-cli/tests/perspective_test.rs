use std::path::PathBuf;
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use pretty_assertions::assert_eq;
use serde_json::Value;

fn idl() -> Command {
    Command::cargo_bin("idl").expect("binary built")
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/test-graph.json")
}

#[test]
fn perspective_markdown_filters_for_product_manager() {
    idl()
        .args(["perspective", "product-manager"])
        .arg(fixture())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "# IDL Perspective: product-manager",
        ))
        .stdout(predicate::str::contains("intent:online-shopping"))
        .stdout(predicate::str::contains("decision:use-event-sourcing"))
        .stdout(predicate::str::contains("entity:order").not());
}

#[test]
fn perspective_json_contains_only_role_nodes_and_surviving_edges() {
    let output = idl()
        .args(["perspective", "security"])
        .arg(fixture())
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: Value = serde_json::from_slice(&output).expect("valid JSON graph");
    let nodes = json["nodes"].as_array().unwrap();
    let kinds: Vec<_> = nodes.iter().map(|n| n["kind"].as_str().unwrap()).collect();
    assert!(kinds.contains(&"policy"));
    assert!(kinds.contains(&"api"));
    assert!(!kinds.contains(&"entity"));

    let edges = json["edges"].as_array().unwrap();
    let edge_kinds: Vec<_> = edges.iter().map(|e| e["kind"].as_str().unwrap()).collect();
    assert_eq!(edges.len(), 4);
    assert!(edge_kinds.contains(&"authorizes"));
    assert!(edge_kinds.contains(&"implements"));
    assert!(edge_kinds.contains(&"constrains"));
}

#[test]
fn perspective_rejects_unknown_role() {
    idl()
        .args(["perspective", "unknown-role"])
        .arg(fixture())
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown perspective role"));
}
