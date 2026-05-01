//! Round runner: builds the LLM request, calls the provider, validates the
//! response, retries up to twice on validation failure, and persists the
//! accepted round into the session.

use crate::prompts::round_prompt;
use crate::session::{Round, Session, SessionStatus};
use crate::validate::{delta_to_graph_doc, DeltaValidator};
use anyhow::{anyhow, Result};
use idl_llm::tools::default_tools;
use idl_llm::{LlmProvider, RoundRequest};
use serde_json::Value;

const MAX_RETRIES: u32 = 2;

#[derive(Debug)]
pub struct RoundOutcome {
    pub round_number: u32,
    pub attempts: u32,
    pub confidence_overall: f64,
    pub questions: usize,
}

pub async fn run_round_with_retries<P: LlmProvider + ?Sized>(
    session: &mut Session,
    provider: &P,
    round_number: u32,
) -> Result<RoundOutcome> {
    let prompt = round_prompt(round_number)
        .ok_or_else(|| anyhow!("no embedded prompt for round {round_number}"))?;
    let validator = DeltaValidator::new()?;

    let prior_graph = session.current_graph();
    let user_payload = serde_json::to_string_pretty(&serde_json::json!({
        "topic": session.topic,
        "session_id": session.id,
        "round": round_number,
        "prior_graph": prior_graph,
        "prior_decisions": collect_decisions(session),
        "outstanding_questions": collect_questions(session)
    }))?;

    let mut last_err: Option<String> = None;

    for attempt in 0..=MAX_RETRIES {
        let request = RoundRequest {
            system: prompt.to_string(),
            user: user_payload.clone(),
            tools: default_tools(),
            round: round_number,
            session_id: session.id.clone(),
            model: session.model.clone(),
            validation_feedback: last_err.clone(),
        };

        let resp = provider.run_round(request).await?;
        let normalized = normalize_anchors(&resp.graph_delta, &session.id, round_number);
        let doc = delta_to_graph_doc(&normalized);

        match validator.validate(&doc) {
            Ok(()) => {
                let questions = resp.questions.len();
                let round = Round {
                    n: round_number,
                    transcript_md: render_transcript(&session.id, round_number, &resp, attempt),
                    graph_delta_json: doc,
                    questions: resp.questions,
                    decisions: resp.decisions,
                    confidence_overall: resp.confidence_overall,
                };
                let confidence = round.confidence_overall;
                session.write_round(round)?;
                if round_number >= session.rounds_planned {
                    session.status = SessionStatus::Completed;
                    session.save()?;
                }
                return Ok(RoundOutcome {
                    round_number,
                    attempts: attempt + 1,
                    confidence_overall: confidence,
                    questions,
                });
            }
            Err(e) => {
                tracing::warn!("round {round_number} attempt {attempt} invalid: {e}");
                last_err = Some(e.0);
            }
        }
    }

    session.status = SessionStatus::Failed;
    session.save()?;
    Err(anyhow!(
        "round {round_number} failed after {} attempts: {}",
        MAX_RETRIES + 1,
        last_err.unwrap_or_else(|| "unknown error".into())
    ))
}

fn collect_decisions(session: &Session) -> Vec<&Value> {
    session.rounds.iter().flat_map(|r| r.decisions.iter()).collect()
}
fn collect_questions(session: &Session) -> Vec<&Value> {
    session.rounds.iter().flat_map(|r| r.questions.iter()).collect()
}

/// Rewrite `interview://session-X/round-N` URIs that the skill prompts model
/// after into the schema-conformant `idl://interview/...` form. Idempotent.
fn normalize_anchors(delta: &Value, session_id: &str, round: u32) -> Value {
    let mut delta = delta.clone();
    if let Some(nodes) = delta.get_mut("nodes").and_then(Value::as_array_mut) {
        for n in nodes {
            if let Some(arr) = n.get_mut("source_anchors").and_then(Value::as_array_mut) {
                for anchor in arr {
                    let new_uri = anchor
                        .get("uri")
                        .and_then(Value::as_str)
                        .and_then(|s| s.strip_prefix("interview://").map(|rest| format!("idl://interview/{rest}")));
                    if let Some(uri) = new_uri {
                        if let Some(obj) = anchor.as_object_mut() {
                            obj.insert("uri".into(), Value::String(uri));
                        }
                    }
                }
            } else {
                // Inject a default anchor if the model omitted one.
                let default = serde_json::json!([{
                    "uri": format!("idl://interview/session-{session_id}/round-{round}")
                }]);
                n.as_object_mut()
                    .unwrap()
                    .insert("source_anchors".into(), default);
            }
        }
    }
    delta
}

fn render_transcript(session_id: &str, round: u32, resp: &idl_llm::RoundResponse, attempt: u32) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Round {round} (session {session_id})\n\n"));
    out.push_str(&format!("- attempts: {}\n", attempt + 1));
    out.push_str(&format!("- confidence_overall: {:.2}\n\n", resp.confidence_overall));
    if !resp.questions.is_empty() {
        out.push_str("## Open questions\n\n");
        for q in &resp.questions {
            if let Some(text) = q.get("question").and_then(Value::as_str) {
                out.push_str(&format!("- {text}\n"));
            }
        }
        out.push('\n');
    }
    if !resp.decisions.is_empty() {
        out.push_str("## Decisions\n\n");
        for d in &resp.decisions {
            let id = d.get("id").and_then(Value::as_str).unwrap_or("?");
            let answer = d.get("answer").and_then(Value::as_str).unwrap_or("");
            out.push_str(&format!("- {id}: {answer}\n"));
        }
        out.push('\n');
    }
    out.push_str("## Graph delta\n\n```json\n");
    out.push_str(&serde_json::to_string_pretty(&resp.graph_delta).unwrap_or_default());
    out.push_str("\n```\n");
    out
}
