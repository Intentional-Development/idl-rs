//! Semantic loss reporting.
//!
//! P0.7 will populate this module. A [`SemanticLossReport`] captures the
//! IDL blocks that could not be losslessly round-tripped through the graph.

use serde::{Deserialize, Serialize};

use crate::provenance::SourceAnchor;

/// One IDL block that was lost or degraded during graph construction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LossEntry {
    pub block: String,
    pub reason: String,
    pub anchor: Option<SourceAnchor>,
}

/// Aggregate semantic-loss report. Populated by P0.7.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SemanticLossReport {
    pub lost_blocks: Vec<LossEntry>,
}
