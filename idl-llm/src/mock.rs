//! Deterministic mock provider that replays canned `RoundResponse` JSON.
//!
//! Tests instantiate [`MockProvider`] with either:
//!   * `from_dir` — load `round-N.json` fixtures from a directory; or
//!   * `from_responses` — pass a `Vec<RoundResponse>` directly.
//!
//! The provider also supports an "invalid-then-valid" mode used to exercise
//! the per-round retry loop: if [`MockProvider::next_invalid`] is set to `n`,
//! the next `n` calls will return a deliberately malformed delta before the
//! recorded fixtures resume.

use crate::{LlmProvider, RoundRequest, RoundResponse};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde_json::json;
use std::path::Path;
use std::sync::Mutex;

pub struct MockProvider {
    by_round: Vec<RoundResponse>,
    invalid_attempts: Mutex<u32>,
}

impl MockProvider {
    pub fn from_responses(responses: Vec<RoundResponse>) -> Self {
        Self { by_round: responses, invalid_attempts: Mutex::new(0) }
    }

    pub fn from_dir(dir: &Path, rounds: u32) -> Result<Self> {
        let mut out = Vec::with_capacity(rounds as usize);
        for n in 1..=rounds {
            let path = dir.join(format!("round-{n}.json"));
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("read fixture {}", path.display()))?;
            let resp: RoundResponse = serde_json::from_str(&text)
                .with_context(|| format!("parse fixture {}", path.display()))?;
            out.push(resp);
        }
        Ok(Self::from_responses(out))
    }

    /// Return malformed deltas for the next `n` calls before the fixtures resume.
    pub fn set_pending_invalid(&self, n: u32) {
        *self.invalid_attempts.lock().unwrap() = n;
    }
}

#[async_trait]
impl LlmProvider for MockProvider {
    async fn run_round(&self, request: RoundRequest) -> Result<RoundResponse> {
        let mut pending = self.invalid_attempts.lock().unwrap();
        if *pending > 0 {
            *pending -= 1;
            // Deliberately invalid: missing required `version` field on the
            // graph_delta and a non-kernel node kind.
            return Ok(RoundResponse {
                graph_delta: json!({
                    "nodes": [{ "id": "bogus:1", "kind": "not_a_kernel_kind" }],
                    "edges": []
                }),
                questions: vec![],
                decisions: vec![],
                confidence_overall: 0.0,
            });
        }
        drop(pending);

        let idx = (request.round as usize)
            .checked_sub(1)
            .ok_or_else(|| anyhow!("round must be >= 1"))?;
        self.by_round
            .get(idx)
            .cloned()
            .ok_or_else(|| anyhow!("MockProvider has no fixture for round {}", request.round))
    }
}
