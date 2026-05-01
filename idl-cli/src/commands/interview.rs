//! `idl interview` — multi-round greenfield interview.
//!
//! Orchestrates [`idl_interview::Session`] and an [`idl_llm::LlmProvider`]
//! (OpenAI by default, Mock when `IDL_INTERVIEW_MOCK_DIR` is set in tests/demos).

use anyhow::{Context, Result};
use idl_interview::session::{locate_intent_dir, Session, SessionStatus};
use idl_interview::{accept, runner};
use idl_llm::mock::MockProvider;
use idl_llm::openai::OpenAiProvider;
use idl_llm::LlmProvider;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

const DEFAULT_MODEL: &str = "gpt-5.5";

fn make_provider(rounds: u32) -> Result<Arc<dyn LlmProvider>> {
    if let Ok(dir) = std::env::var("IDL_INTERVIEW_MOCK_DIR") {
        let mock = MockProvider::from_dir(&PathBuf::from(dir), rounds)
            .context("load MockProvider fixtures")?;
        return Ok(Arc::new(mock));
    }
    let p = OpenAiProvider::from_env()
        .context("OpenAI provider unavailable. Set OPENAI_API_KEY or IDL_INTERVIEW_MOCK_DIR.")?;
    Ok(Arc::new(p))
}

pub fn run_new(topic: String, rounds: u32) -> Result<ExitCode> {
    let cwd = std::env::current_dir()?;
    let intent = locate_intent_dir(&cwd)?;
    let mut session = Session::new(&intent, topic, DEFAULT_MODEL, rounds)?;
    println!("✓ session created: {}", session.id);

    let provider = make_provider(rounds)?;
    let outcome = tokio::runtime::Runtime::new()?
        .block_on(runner::run_round_with_retries(&mut session, provider.as_ref(), 1))?;
    println!(
        "  round {} done (attempts={}, confidence={:.2}, questions={})",
        outcome.round_number, outcome.attempts, outcome.confidence_overall, outcome.questions
    );
    Ok(ExitCode::from(0))
}

pub fn run_continue(session_id: String) -> Result<ExitCode> {
    let cwd = std::env::current_dir()?;
    let intent = locate_intent_dir(&cwd)?;
    let mut session = Session::load(&intent, &session_id)?;
    if matches!(session.status, SessionStatus::Completed | SessionStatus::Accepted) {
        println!("session {session_id} already {:?}; nothing to do", session.status);
        return Ok(ExitCode::from(0));
    }
    let next = session.next_round_number();
    if next > session.rounds_planned {
        println!("all {} rounds already completed", session.rounds_planned);
        return Ok(ExitCode::from(0));
    }
    let provider = make_provider(session.rounds_planned)?;
    let outcome = tokio::runtime::Runtime::new()?
        .block_on(runner::run_round_with_retries(&mut session, provider.as_ref(), next))?;
    println!(
        "✓ round {} done (attempts={}, confidence={:.2}, questions={})",
        outcome.round_number, outcome.attempts, outcome.confidence_overall, outcome.questions
    );
    Ok(ExitCode::from(0))
}

pub fn run_accept(session_id: String) -> Result<ExitCode> {
    let cwd = std::env::current_dir()?;
    let intent = locate_intent_dir(&cwd)?;
    let mut session = Session::load(&intent, &session_id)?;
    let outcome = accept::accept(&intent, &mut session)?;
    println!(
        "✓ promoted to {} ({} nodes, {} edges) at {}",
        outcome.change_id,
        outcome.node_count,
        outcome.edge_count,
        outcome.folder.display()
    );
    Ok(ExitCode::from(0))
}

pub fn run_list() -> Result<ExitCode> {
    let cwd = std::env::current_dir()?;
    let intent = locate_intent_dir(&cwd)?;
    let sessions = Session::list(&intent)?;
    if sessions.is_empty() {
        println!("no interview sessions yet");
        return Ok(ExitCode::from(0));
    }
    println!("interview sessions:");
    for s in sessions {
        let last_conf = s.rounds.last().map(|r| r.confidence_overall).unwrap_or(0.0);
        println!(
            "  {}  rounds={}/{}  status={:?}  conf={:.2}  topic={:?}",
            s.id,
            s.rounds.len(),
            s.rounds_planned,
            s.status,
            last_conf,
            s.topic
        );
    }
    Ok(ExitCode::from(0))
}

pub fn run_show(session_id: String) -> Result<ExitCode> {
    let cwd = std::env::current_dir()?;
    let intent = locate_intent_dir(&cwd)?;
    let session = Session::load(&intent, &session_id)?;
    println!(
        "session {} (topic: {:?}, status: {:?}, rounds: {}/{})",
        session.id, session.topic, session.status, session.rounds.len(), session.rounds_planned
    );
    for r in &session.rounds {
        println!("---\n{}", r.transcript_md);
    }
    let graph = session.current_graph();
    let n = graph.get("nodes").and_then(|v| v.as_array()).map(Vec::len).unwrap_or(0);
    let e = graph.get("edges").and_then(|v| v.as_array()).map(Vec::len).unwrap_or(0);
    println!("=== Accumulated graph: {n} nodes, {e} edges ===");
    println!("{}", serde_json::to_string_pretty(&graph)?);
    Ok(ExitCode::from(0))
}
