//! Node lifecycle state machine.
//!
//! These six variants are **mandatory** per Wave 8 feedback and lock the
//! contract that downstream emitters, validators, and the AI Kernel must
//! respect. Do not add or remove variants without an explicit decision drop.

use serde::{Deserialize, Serialize};

/// Lifecycle state of a graph node.
///
/// The six variants below are the canonical state set:
///
/// - `Accepted`  — confirmed by a human or trusted source.
/// - `Proposed`  — suggested by tooling or a contributor, awaiting review.
/// - `Inferred`  — derived by the AI Kernel; carries a [`crate::Confidence`].
/// - `Questioned`— flagged for review; semantics unclear or contested.
/// - `Rejected`  — explicitly refused; retained for provenance.
/// - `Drifted`   — no longer matches its source; needs reconciliation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeState {
    Accepted,
    Proposed,
    Inferred,
    Questioned,
    Rejected,
    Drifted,
}
