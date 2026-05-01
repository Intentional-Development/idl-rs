//! Graph node type.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::kind::NodeKind;
use crate::provenance::{Confidence, SourceAnchor};
use crate::state::NodeState;
use crate::Value;

/// Stable identifier for a [`Node`] within a [`crate::Graph`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeId(pub String);

/// A node in the IDL property graph.
///
/// Each node carries its lifecycle [`NodeState`], typed [`NodeKind`],
/// arbitrary `props`, the [`SourceAnchor`]s that produced it, and an
/// optional model [`Confidence`] when inferred.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub state: NodeState,
    pub props: BTreeMap<String, Value>,
    pub source_anchors: Vec<SourceAnchor>,
    pub confidence: Option<Confidence>,
}
