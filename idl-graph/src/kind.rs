//! Node and edge taxonomy.
//!
//! TODO: populated by P0.6 AI Kernel decision (in progress).
//! The full variant list is intentionally deferred until the kernel decision
//! lands; both enums are `#[non_exhaustive]` so adding variants is non-breaking.

use serde::{Deserialize, Serialize};

/// Type tag for a [`crate::Node`].
///
/// TODO: populated by P0.6 AI Kernel decision (in progress).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    /// Placeholder until P0.6 lands. Will be removed once the real taxonomy
    /// is decided.
    Unknown,
}

/// Type tag for an [`crate::Edge`].
///
/// TODO: populated by P0.6 AI Kernel decision (in progress).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// Placeholder until P0.6 lands.
    Unknown,
}
