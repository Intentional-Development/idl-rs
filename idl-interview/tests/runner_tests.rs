//! End-to-end tests for the interview runtime using `MockProvider`.

use idl_interview::accept;
use idl_interview::runner::run_round_with_retries;
use idl_interview::session::{Session, SessionStatus};
use idl_llm::mock::MockProvider;
use idl_llm::RoundResponse;
use std::path::{Path, PathBuf};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/demo-todo-app")
}

fn intent_dir(tmp: &Path) -> PathBuf {
    let i = tmp.join("intent");
    std::fs::create_dir_all(i.join("changes")).unwrap();
    i
}

fn load_demo_responses() -> Vec<RoundResponse> {
    (1..=5)
        .map(|n| {
            let p = fixtures_dir().join(format!("round-{n}.json"));
            let s = std::fs::read_to_string(&p).unwrap();
            serde_json::from_str(&s).unwrap()
        })
        .collect()
}

#[tokio::test]
async fn interview_runs_5_rounds_to_completion() {
    let tmp = tempfile::tempdir().unwrap();
    let intent = intent_dir(tmp.path());
    let provider = MockProvider::from_responses(load_demo_responses());

    let mut session = Session::new(&intent, "todo app for solo users", "gpt-5.5", 5).unwrap();

    for n in 1..=5 {
        let outcome = run_round_with_retries(&mut session, &provider, n).await.unwrap();
        assert_eq!(outcome.attempts, 1, "round {n} should pass on first attempt");
    }
    assert_eq!(session.status, SessionStatus::Completed);
    assert_eq!(session.rounds.len(), 5);

    let graph = session.current_graph();
    let nodes = graph.get("nodes").unwrap().as_array().unwrap().len();
    // Demo: ~12-16 kernel nodes (decision-heavy variant); accept anywhere in that band.
    assert!(nodes >= 12, "expected ≥12 accumulated nodes, got {nodes}");
}

#[tokio::test]
async fn invalid_delta_retries_then_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let intent = intent_dir(tmp.path());

    // Provider returns valid round 1 fixture, but we force three consecutive
    // invalid attempts (max retries = 2 → 3 total attempts → all invalid).
    let provider = MockProvider::from_responses(load_demo_responses());
    provider.set_pending_invalid(3);

    let mut session = Session::new(&intent, "demo", "gpt-5.5", 5).unwrap();
    let res = run_round_with_retries(&mut session, &provider, 1).await;
    assert!(res.is_err(), "round should fail after 3 invalid attempts");
    let session2 = Session::load(&intent, &session.id).unwrap();
    assert_eq!(session2.status, SessionStatus::Failed);
}

#[tokio::test]
async fn one_invalid_then_recovers() {
    let tmp = tempfile::tempdir().unwrap();
    let intent = intent_dir(tmp.path());
    let provider = MockProvider::from_responses(load_demo_responses());
    provider.set_pending_invalid(1);

    let mut session = Session::new(&intent, "demo", "gpt-5.5", 5).unwrap();
    let outcome = run_round_with_retries(&mut session, &provider, 1).await.unwrap();
    assert_eq!(outcome.attempts, 2, "should succeed on the second attempt");
}

#[tokio::test]
async fn accept_creates_proposed_change_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let intent = intent_dir(tmp.path());
    let provider = MockProvider::from_responses(load_demo_responses());

    let mut session = Session::new(&intent, "todo app for solo users", "gpt-5.5", 5).unwrap();
    for n in 1..=5 {
        run_round_with_retries(&mut session, &provider, n).await.unwrap();
    }

    let outcome = accept::accept(&intent, &mut session).unwrap();
    assert!(outcome.folder.exists());
    assert!(outcome.folder.join("state.json").exists());
    assert!(outcome.folder.join("delta.json").exists());
    assert!(outcome.folder.join("decisions.md").exists());
    assert!(outcome.folder.join("sources.json").exists());
    assert!(outcome.folder.join("verifications/plan.md").exists());
    assert!(outcome.node_count >= 12);

    let state: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(outcome.folder.join("state.json")).unwrap())
            .unwrap();
    assert_eq!(state["state"], "proposed");
    assert_eq!(session.status, SessionStatus::Accepted);
    assert!(session.promoted_change_id.is_some());
}
