//! `idl-llm` — minimal LLM provider abstraction for `idl interview`.
//!
//! Defines the [`LlmProvider`] async trait, the [`RoundRequest`] / [`RoundResponse`]
//! data carriers shared between the CLI and the model, the tool-definition catalog
//! mandated by `IDL/skills/idl-interview/codex-runtime.md`, and two reference
//! implementations:
//!
//! * [`openai::OpenAiProvider`] — talks to the OpenAI Responses API via reqwest.
//!   The HTTP transport is wrapped in the [`openai::HttpClient`] trait so unit
//!   tests can swap in a fake without touching the network.
//! * [`mock::MockProvider`] — replays deterministic JSON fixtures keyed by
//!   round number; used by `idl-interview` tests and the `EXAMPLE.md` demo.

pub mod mock;
pub mod openai;
pub mod tools;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundRequest {
    pub system: String,
    pub user: String,
    pub tools: Vec<ToolDef>,
    /// Round number (1..=N) — providers may use it to select fixtures or templates.
    pub round: u32,
    /// Stable session id, exposed so providers can echo it in run metadata.
    pub session_id: String,
    /// Model identifier (default `gpt-5.5` per skill spec).
    pub model: String,
    /// Optional validation feedback from a prior failed attempt.
    pub validation_feedback: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundResponse {
    pub graph_delta: Value,
    #[serde(default)]
    pub questions: Vec<Value>,
    #[serde(default)]
    pub decisions: Vec<Value>,
    #[serde(default)]
    pub confidence_overall: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn run_round(&self, request: RoundRequest) -> anyhow::Result<RoundResponse>;
}
