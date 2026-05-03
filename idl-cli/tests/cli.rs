use std::path::PathBuf;
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::TempDir;

fn idl() -> Command {
    Command::cargo_bin("idl").expect("binary built")
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[test]
fn version_flag_prints_crate_version() {
    idl()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn init_greenfield_scaffolds_layout() {
    let tmp = TempDir::new().unwrap();
    idl()
        .args(["init", "--greenfield", "--dir"])
        .arg(tmp.path())
        .assert()
        .success();

    let intent = tmp.path().join("intent");
    assert!(intent.join("project.idl").is_file());
    assert!(intent.join(".idl/config.json").is_file());
    assert!(intent
        .join("changes/0001-initial-intent/state.json")
        .is_file());
    assert!(intent
        .join("changes/0001-initial-intent/intent-delta.idl")
        .is_file());
    assert!(intent
        .join("changes/0001-initial-intent/decisions.md")
        .is_file());

    let cfg = std::fs::read_to_string(intent.join(".idl/config.json")).unwrap();
    assert!(cfg.contains("\"mode\": \"greenfield\""));
}

#[test]
fn init_brownfield_scaffolds_extracted_placeholder() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src");
    std::fs::create_dir_all(&src).unwrap();

    idl()
        .args(["init", "--brownfield", "--source"])
        .arg(&src)
        .arg("--dir")
        .arg(tmp.path())
        .assert()
        .success();

    let intent = tmp.path().join("intent");
    assert!(intent.join("extracted/source-manifest.json").is_file());
    assert!(intent
        .join("changes/0001-promote-extraction/state.json")
        .is_file());
}

#[test]
fn change_new_scaffolds_change_folder() {
    let tmp = TempDir::new().unwrap();
    idl()
        .args(["init", "--greenfield", "--dir"])
        .arg(tmp.path())
        .assert()
        .success();

    idl()
        .current_dir(tmp.path())
        .args(["change", "new", "test-slug"])
        .assert()
        .success();

    let folder = tmp.path().join("intent/changes/0002-test-slug");
    assert!(folder.is_dir(), "expected {} to exist", folder.display());
    assert!(folder.join("state.json").is_file());
    assert!(folder.join("intent-delta.idl").is_file());
    assert!(folder.join("decisions.md").is_file());

    // `change list` lists both folders.
    idl()
        .current_dir(tmp.path())
        .args(["change", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("0001-initial-intent"))
        .stdout(predicate::str::contains("0002-test-slug"));
}

#[test]
fn change_new_rejects_bad_slug() {
    let tmp = TempDir::new().unwrap();
    idl()
        .args(["init", "--greenfield", "--dir"])
        .arg(tmp.path())
        .assert()
        .success();

    idl()
        .current_dir(tmp.path())
        .args(["change", "new", "Bad_Slug"])
        .assert()
        .failure();
}

#[test]
fn validate_ok_fixture_succeeds() {
    idl()
        .arg("validate")
        .arg(fixtures().join("ok.idl"))
        .assert()
        .code(predicate::in_iter(vec![0i32, 2]))
        .stdout(predicate::str::contains("coverage:"));
}

#[test]
fn validate_broken_fixture_fails() {
    idl()
        .arg("validate")
        .arg(fixtures().join("broken.idl"))
        .assert()
        .failure();
}

#[test]
fn validate_json_emits_machine_output() {
    idl()
        .arg("validate")
        .arg("--json")
        .arg(fixtures().join("ok.idl"))
        .assert()
        .stdout(predicate::str::contains("\"coverage_pct\""));
}

#[test]
fn extract_is_scaffold_only() {
    idl()
        .args(["extract", "--source", "."])
        .assert()
        .success()
        .stdout(predicate::str::contains("TODO"));
}

#[test]
fn change_propose_is_stub() {
    let tmp = TempDir::new().unwrap();
    idl()
        .args(["init", "--greenfield", "--dir"])
        .arg(tmp.path())
        .assert()
        .success();
    idl()
        .current_dir(tmp.path())
        .args(["change", "propose", "0001-initial-intent"])
        .assert()
        .success()
        .stdout(predicate::str::contains("TODO"));
}
