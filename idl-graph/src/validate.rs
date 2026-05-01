//! Graph constraint validation.
//!
//! P0.3 starter constraint set. Each [`Constraint`] is a pure function over
//! a [`Graph`] returning the violations it found. Severity is attached to
//! every violation so callers can decide what is fatal vs informational.

use serde::{Deserialize, Serialize};

use crate::edge::EdgeId;
use crate::kind::{EdgeKind, NodeKind};
use crate::node::NodeId;
use crate::state::NodeState;
use crate::Graph;

/// Severity level attached to a [`ConstraintViolation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warn,
    Info,
}

/// A single constraint violation surfaced by [`Graph::validate`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConstraintViolation {
    pub rule: String,
    pub severity: Severity,
    pub message: String,
    pub node: Option<NodeId>,
    pub edge: Option<EdgeId>,
}

impl ConstraintViolation {
    pub fn error(rule: &str, message: impl Into<String>) -> Self {
        Self {
            rule: rule.to_string(),
            severity: Severity::Error,
            message: message.into(),
            node: None,
            edge: None,
        }
    }

    pub fn warn(rule: &str, message: impl Into<String>) -> Self {
        Self {
            rule: rule.to_string(),
            severity: Severity::Warn,
            message: message.into(),
            node: None,
            edge: None,
        }
    }

    pub fn with_node(mut self, n: NodeId) -> Self {
        self.node = Some(n);
        self
    }

    pub fn with_edge(mut self, e: EdgeId) -> Self {
        self.edge = Some(e);
        self
    }
}

/// Aggregate validation report grouping violations by severity.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationReport {
    /// Names of the constraints that ran.
    pub checked: Vec<String>,
    pub errors: Vec<ConstraintViolation>,
    pub warnings: Vec<ConstraintViolation>,
    pub infos: Vec<ConstraintViolation>,
}

impl ValidationReport {
    pub fn push(&mut self, v: ConstraintViolation) {
        match v.severity {
            Severity::Error => self.errors.push(v),
            Severity::Warn => self.warnings.push(v),
            Severity::Info => self.infos.push(v),
        }
    }

    /// `true` iff no error-level violations were collected.
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn total(&self) -> usize {
        self.errors.len() + self.warnings.len() + self.infos.len()
    }
}

/// A single graph constraint.
pub trait Constraint: Send + Sync {
    /// Stable identifier (used in [`ConstraintViolation::rule`] and reports).
    fn name(&self) -> &str;

    /// Evaluate over `graph` and return any violations.
    fn check(&self, graph: &Graph) -> Vec<ConstraintViolation>;
}

// ---------------------------------------------------------------------------
// Starter constraint set (P0.3)
// ---------------------------------------------------------------------------

/// Every node with `state == Accepted` must have ≥1 `source_anchors` OR a
/// recorded `decision` reference (incoming `Decides` edge from a `Decision`).
pub struct AcceptedNodesHaveProvenance;

impl Constraint for AcceptedNodesHaveProvenance {
    fn name(&self) -> &str {
        "accepted-nodes-have-provenance"
    }

    fn check(&self, graph: &Graph) -> Vec<ConstraintViolation> {
        let mut out = Vec::new();
        for node in graph.iter_nodes_by_state(NodeState::Accepted) {
            if !node.source_anchors.is_empty() {
                continue;
            }
            let decided_by = graph.iter_edges_to(&node.id).any(|e| {
                e.kind == EdgeKind::Decides
                    && graph
                        .get_node(&e.from)
                        .map(|n| n.kind == NodeKind::Decision)
                        .unwrap_or(false)
            });
            if decided_by {
                continue;
            }
            out.push(
                ConstraintViolation::error(
                    self.name(),
                    format!(
                        "accepted node {:?} has no source_anchors and no incoming `decides` edge from a decision",
                        node.id.0
                    ),
                )
                .with_node(node.id.clone()),
            );
        }
        out
    }
}

/// Every node with `state == Proposed` must carry a [`crate::Confidence`]
/// (enforces the AI-origin invariant: proposed-by-tooling without confidence
/// is treated as a contract violation).
pub struct ProposedNodesHaveConfidence;

impl Constraint for ProposedNodesHaveConfidence {
    fn name(&self) -> &str {
        "proposed-nodes-have-confidence"
    }

    fn check(&self, graph: &Graph) -> Vec<ConstraintViolation> {
        let mut out = Vec::new();
        for node in graph.iter_nodes_by_state(NodeState::Proposed) {
            if node.confidence.is_none() {
                out.push(
                    ConstraintViolation::error(
                        self.name(),
                        format!("proposed node {:?} has no confidence record", node.id.0),
                    )
                    .with_node(node.id.clone()),
                );
            }
        }
        out
    }
}

/// Every node's `kind` must be a kernel variant. Today the [`NodeKind`] enum
/// is kernel-only by construction so this is a no-op safeguard against future
/// drift; emits warnings rather than errors so an extension layer can opt in.
pub struct KernelKindOnly;

impl Constraint for KernelKindOnly {
    fn name(&self) -> &str {
        "kernel-kind-only"
    }

    fn check(&self, graph: &Graph) -> Vec<ConstraintViolation> {
        let mut out = Vec::new();
        for node in graph.iter_nodes() {
            if !node.kind.is_kernel() {
                out.push(
                    ConstraintViolation {
                        rule: self.name().to_string(),
                        severity: Severity::Warn,
                        message: format!(
                            "node {:?} uses non-kernel kind {}",
                            node.id.0,
                            node.kind.canonical()
                        ),
                        node: Some(node.id.clone()),
                        edge: None,
                    },
                );
            }
        }
        out
    }
}

/// Every edge's `from`/`to` must resolve to existing nodes.
pub struct EdgeEndpointsExist;

impl Constraint for EdgeEndpointsExist {
    fn name(&self) -> &str {
        "edge-endpoints-exist"
    }

    fn check(&self, graph: &Graph) -> Vec<ConstraintViolation> {
        let mut out = Vec::new();
        for edge in graph.iter_edges() {
            if graph.get_node(&edge.from).is_none() {
                out.push(
                    ConstraintViolation::error(
                        self.name(),
                        format!(
                            "edge {:?} references missing source node {:?}",
                            edge.id.0, edge.from.0
                        ),
                    )
                    .with_edge(edge.id.clone()),
                );
            }
            if graph.get_node(&edge.to).is_none() {
                out.push(
                    ConstraintViolation::error(
                        self.name(),
                        format!(
                            "edge {:?} references missing target node {:?}",
                            edge.id.0, edge.to.0
                        ),
                    )
                    .with_edge(edge.id.clone()),
                );
            }
        }
        out
    }
}

/// `trace_link` nodes must have ≥1 outgoing `TracesTo` edge.
pub struct NoDanglingTraceLinks;

impl Constraint for NoDanglingTraceLinks {
    fn name(&self) -> &str {
        "no-dangling-trace-links"
    }

    fn check(&self, graph: &Graph) -> Vec<ConstraintViolation> {
        let mut out = Vec::new();
        for node in graph.query_by_kind(&NodeKind::TraceLink) {
            let has_out = graph
                .iter_edges_from(&node.id)
                .any(|e| e.kind == EdgeKind::TracesTo);
            if !has_out {
                out.push(
                    ConstraintViolation::error(
                        self.name(),
                        format!(
                            "trace_link {:?} has no outgoing `traces_to` edge",
                            node.id.0
                        ),
                    )
                    .with_node(node.id.clone()),
                );
            }
        }
        out
    }
}

/// Every `Accepted` `rule`/`invariant` should have ≥1 incoming `Verifies`
/// edge from a `verification` node. Warning level — coverage signal, not
/// hard contract.
pub struct VerificationCovers;

impl Constraint for VerificationCovers {
    fn name(&self) -> &str {
        "verification-covers"
    }

    fn check(&self, graph: &Graph) -> Vec<ConstraintViolation> {
        let mut out = Vec::new();
        for node in graph.iter_nodes_by_state(NodeState::Accepted) {
            if !matches!(node.kind, NodeKind::Rule | NodeKind::Invariant) {
                continue;
            }
            let verified = graph.iter_edges_to(&node.id).any(|e| {
                e.kind == EdgeKind::Verifies
                    && graph
                        .get_node(&e.from)
                        .map(|n| n.kind == NodeKind::Verification)
                        .unwrap_or(false)
            });
            if !verified {
                out.push(
                    ConstraintViolation::warn(
                        self.name(),
                        format!(
                            "{} {:?} has no incoming `verifies` edge from a verification node",
                            node.kind.canonical(),
                            node.id.0
                        ),
                    )
                    .with_node(node.id.clone()),
                );
            }
        }
        out
    }
}

/// The 6 starter constraints, ready to drop into [`Graph::validate`].
pub fn default_constraints() -> Vec<Box<dyn Constraint>> {
    vec![
        Box::new(AcceptedNodesHaveProvenance),
        Box::new(ProposedNodesHaveConfidence),
        Box::new(KernelKindOnly),
        Box::new(EdgeEndpointsExist),
        Box::new(NoDanglingTraceLinks),
        Box::new(VerificationCovers),
    ]
}
