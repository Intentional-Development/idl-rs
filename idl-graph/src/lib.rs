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

pub mod doc;
pub mod drift;
pub mod edge;
pub mod extensions_dto;
pub mod extensions_when;
pub mod graph;
pub mod kind;
pub mod loss;
pub mod node;
pub mod provenance;
pub mod state;
pub mod validate;

pub use doc::{ConfidenceDoc, EdgeDoc, GraphDoc, NodeDoc, RangeDoc, SourceAnchorDoc};
pub use drift::{
    diff_against_source, diff_against_sources, diff_graphs, AnchorEntry, AnchorReport,
    AnchorVerdict, DriftEntry, DriftEvent, DriftReport, DriftSeverity, PropChange,
};
#[allow(deprecated)]
pub use extensions_dto::{
    compute_projected_fields, parse_dtos, project_field_set, projected_fields_ordered,
    validate_dtos, DtoDefinition, DtoExtra, DtoKind, DtoViolation, ProjectedField,
};
pub use extensions_when::{parse_when, When, WhenStructured, WhenVar};

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
