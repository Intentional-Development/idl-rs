//! Graph constraint validation.
//!
//! P0.3 graph constraints — to be populated.

use serde::{Deserialize, Serialize};

use crate::edge::EdgeId;
use crate::node::NodeId;
use crate::Graph;

/// A single constraint violation surfaced by [`Graph::validate`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConstraintViolation {
    pub rule: String,
    pub message: String,
    pub node: Option<NodeId>,
    pub edge: Option<EdgeId>,
}

/// Aggregate validation report (passing constraints, warnings, stats).
///
/// P0.3 graph constraints — to be populated.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Report {
    pub checked: usize,
    pub passed: usize,
}

/// A single graph constraint.
///
/// P0.3 graph constraints — to be populated.
pub trait Constraint: Send + Sync {
    /// Stable identifier for the rule (used in [`ConstraintViolation::rule`]).
    fn id(&self) -> &str;

    /// Evaluate the constraint over `graph`, pushing any violations into `out`.
    fn check(&self, graph: &Graph, out: &mut Vec<ConstraintViolation>);
}
