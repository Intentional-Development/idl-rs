use std::path::PathBuf;
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::TempDir;

fn idl() -> Command {
    Command::cargo_bin("idl").expect("binary built")
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/test-graph.json")
}

#[test]
fn prompts_generates_all_assistant_files() {
    let tmp = TempDir::new().unwrap();

    idl()
        .args(["prompts"])
        .arg(fixture())
        .args(["--target", "all", "--out-dir"])
        .arg(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(".cursorrules"))
        .stdout(predicate::str::contains("copilot-instructions.md"))
        .stdout(predicate::str::contains("CLAUDE.md"));

    let cursor = std::fs::read_to_string(tmp.path().join(".cursorrules")).unwrap();
    assert!(cursor.contains("Order"));
    assert!(cursor.contains("OrderDto"));
    assert!(cursor.contains("Orders API"));
    assert!(cursor.contains("Positive Quantity"));

    let copilot = tmp.path().join(".github/copilot-instructions.md");
    assert!(copilot.is_file());
    assert!(tmp.path().join("CLAUDE.md").is_file());
}

#[test]
fn prompts_generates_single_target() {
    let tmp = TempDir::new().unwrap();

    idl()
        .args(["prompts"])
        .arg(fixture())
        .args(["--target", "copilot", "--out-dir"])
        .arg(tmp.path())
        .assert()
        .success();

    assert!(tmp.path().join(".github/copilot-instructions.md").is_file());
    assert!(!tmp.path().join(".cursorrules").exists());
    assert!(!tmp.path().join("CLAUDE.md").exists());
}

#[test]
fn prompts_rejects_unknown_target() {
    let tmp = TempDir::new().unwrap();

    idl()
        .args(["prompts"])
        .arg(fixture())
        .args(["--target", "vim", "--out-dir"])
        .arg(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown prompt target"));
}
