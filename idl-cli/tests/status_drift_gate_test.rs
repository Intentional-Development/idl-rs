use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use serde_json::json;
use tempfile::TempDir;

fn idl() -> Command {
    Command::cargo_bin("idl").expect("binary built")
}

fn write_graph(dir: &Path, version: &str) -> PathBuf {
    let graph = json!({
        "version": version,
        "metadata": {
            "name": "Status Fixture",
            "emit_targets": ["typescript", "python", "rust"],
            "conformance": { "corpora": ["fixture"] }
        },
        "nodes": [
            {
                "id": "entity:user",
                "kind": "entity",
                "state": "accepted",
                "props": {
                    "name": "User",
                    "attributes": [
                        { "name": "id", "type": "string" },
                        { "name": "age", "type": "int", "nullable": true }
                    ]
                }
            },
            {
                "id": "operation:create-user",
                "kind": "operation",
                "state": "accepted",
                "props": {
                    "name": "Create User",
                    "inputs": [{ "name": "name", "type": "string" }],
                    "outputs": [{ "name": "user", "type": "User" }]
                }
            }
        ],
        "edges": []
    });
    let path = dir.join("idl.graph.json");
    fs::write(&path, serde_json::to_string_pretty(&graph).unwrap()).unwrap();
    path
}

fn seed_baseline(dir: &Path, graph: &Path) {
    for target in ["typescript", "python", "rust"] {
        idl()
            .current_dir(dir)
            .args(["emit", target])
            .arg(graph)
            .arg("--out")
            .arg(dir.join("generated").join(target))
            .assert()
            .success();
    }
}

#[test]
fn status_errors_without_graph() {
    let temp = TempDir::new().unwrap();
    idl()
        .current_dir(temp.path())
        .arg("status")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no graph file found"));
}

#[test]
fn status_happy_path_reports_sections() {
    let temp = TempDir::new().unwrap();
    write_graph(temp.path(), "0.1.9");
    fs::create_dir_all(temp.path().join("changes/proposals")).unwrap();
    fs::write(
        temp.path()
            .join("changes/proposals/20261230120000-pending.json"),
        serde_json::to_string_pretty(&json!({
            "version": "0.1.0",
            "id": "20261230120000-pending",
            "author": "test",
            "target_graph": "idl.graph.json",
            "rationale": "pending",
            "diff_ops": [],
            "status": "pending",
            "created_at": "2026-12-30T12:00:00Z"
        }))
        .unwrap(),
    )
    .unwrap();
    fs::create_dir_all(temp.path().join("conformance/fixture")).unwrap();
    fs::write(
        temp.path()
            .join("conformance/fixture/conformance-report.json"),
        serde_json::to_string_pretty(&json!({ "corpus": "fixture", "summary": "PASS 1/1" }))
            .unwrap(),
    )
    .unwrap();

    idl()
        .current_dir(temp.path())
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("Workspace"))
        .stdout(predicate::str::contains("pending: 1"))
        .stdout(predicate::str::contains("fixture: PASS 1/1"));
}

#[test]
fn status_json_output_is_machine_readable() {
    let temp = TempDir::new().unwrap();
    write_graph(temp.path(), "0.1.9");
    let output = idl()
        .current_dir(temp.path())
        .args(["status", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["workspace"]["schema_version"], "0.1.9");
    assert_eq!(value["schema"]["matches"], true);
}

#[test]
fn status_warns_on_mismatched_schema_version() {
    let temp = TempDir::new().unwrap();
    write_graph(temp.path(), "0.1.0");
    idl()
        .current_dir(temp.path())
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("warning: schema version mismatch"));
}

#[test]
fn drift_gate_clean_workspace_exits_zero() {
    let temp = TempDir::new().unwrap();
    let graph = write_graph(temp.path(), "0.1.9");
    seed_baseline(temp.path(), &graph);

    idl()
        .current_dir(temp.path())
        .args(["drift", "--gate"])
        .assert()
        .success()
        .stdout(predicate::str::contains("verdict: clean"));
}

#[test]
fn drift_gate_seeded_target_drift_exits_one() {
    let temp = TempDir::new().unwrap();
    let graph = write_graph(temp.path(), "0.1.9");
    seed_baseline(temp.path(), &graph);
    fs::write(
        temp.path()
            .join("generated/python/idl_generated/entities.py"),
        "# drifted\n",
    )
    .unwrap();

    idl()
        .current_dir(temp.path())
        .args(["drift", "--gate", "--json"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("\"verdict\": \"drifted\""))
        .stdout(predicate::str::contains("python"));
}
