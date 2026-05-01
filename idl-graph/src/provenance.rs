//! Provenance: where a node/edge came from and how confident we are.

use serde::{Deserialize, Serialize};

/// Pointer back to the source artifact that produced a node or edge.
///
/// `range` is a byte offset pair `(start, end)` when known. `hash` is an
/// optional content hash (algorithm TBD with P0.1) used to detect drift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceAnchor {
    pub uri: String,
    pub range: Option<(usize, usize)>,
    pub hash: Option<String>,
}

/// Confidence record attached to nodes/edges produced by an AI model.
///
/// `score` is in `[0.0, 1.0]`; `model` and `run_id` identify the producer
/// for reproducibility.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Confidence {
    pub score: f32,
    pub model: String,
    pub run_id: String,
}
