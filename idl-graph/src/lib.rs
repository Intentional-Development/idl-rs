//! # idl-graph
//!
//! Canonical property-graph contract for the IDL toolchain.
//!
//! This crate owns the in-memory representation of the IDL semantic graph:
//! typed [`Node`]s and [`Edge`]s with provenance, a 6-state lifecycle
//! ([`NodeState`]), the 18-construct AI kernel taxonomy, constraint
//! validation, and semantic-loss reporting.
//!
//! ## Roadmap
//! - **P0.1** — `semantic-graph.schema.json` mirrors the types defined here.
//! - **P0.3** — graph constraints in [`validate`].
//! - **P0.6** — AI Kernel decision frozen in [`kind`].
//! - **P0.7** — semantic loss reporting in [`loss`].

pub mod edge;
pub mod graph;
pub mod kind;
pub mod loss;
pub mod node;
pub mod provenance;
pub mod state;
pub mod validate;

pub use edge::{Edge, EdgeId};
pub use graph::Graph;
pub use kind::{EdgeKind, NodeKind, ParseKindError};
pub use loss::{LossEntry, LossReason, SemanticLossReport};
pub use node::{Node, NodeId};
pub use provenance::{Confidence, SourceAnchor};
pub use state::NodeState;
pub use validate::{
    default_constraints, AcceptedNodesHaveProvenance, Constraint, ConstraintViolation,
    EdgeEndpointsExist, KernelKindOnly, NoDanglingTraceLinks, ProposedNodesHaveConfidence,
    Severity, ValidationReport, VerificationCovers,
};

/// JSON value re-export used for free-form node/edge properties.
pub type Value = serde_json::Value;
