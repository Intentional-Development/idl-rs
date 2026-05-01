//! Graph edge type.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::kind::EdgeKind;
use crate::node::NodeId;
use crate::Value;

/// Stable identifier for an [`Edge`] within a [`crate::Graph`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EdgeId(pub String);

/// A directed, typed edge connecting two [`crate::Node`]s.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: EdgeId,
    pub kind: EdgeKind,
    pub from: NodeId,
    pub to: NodeId,
    pub props: BTreeMap<String, Value>,
}
