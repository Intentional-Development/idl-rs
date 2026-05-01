//! `idl-interview` — multi-round greenfield interview runtime.
//!
//! This crate owns:
//!   * The on-disk session layout (`intent/.idl/interview/sessions/<id>/`).
//!   * The accumulator that merges per-round graph deltas into a current graph.
//!   * The schema validator that checks each round's delta before persistence.
//!   * The runner loop with a bounded retry on validation failures.
//!   * The `accept` flow that promotes the accumulated delta into a
//!     `intent/changes/NNNN-<slug>/` proposed change folder.
//!
//! It is provider-agnostic: the LLM is an [`idl_llm::LlmProvider`] passed in
//! by the CLI, which makes it trivial to swap [`idl_llm::mock::MockProvider`]
//! into tests and the demo while production builds use the real OpenAI
//! provider.

pub mod accept;
pub mod prompts;
pub mod runner;
pub mod session;
pub mod validate;

pub use runner::{run_round_with_retries, RoundOutcome};
pub use session::{Round, Session, SessionStatus};
