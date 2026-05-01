//! In-memory property graph container.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::edge::{Edge, EdgeId};
use crate::kind::NodeKind;
use crate::node::{Node, NodeId};
use crate::validate::{Constraint, ConstraintViolation, Report};

/// The IDL semantic property graph.
///
/// Bodies are intentionally unimplemented; this is the P0.1 type skeleton.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Graph {
    pub nodes: HashMap<NodeId, Node>,
    pub edges: HashMap<EdgeId, Edge>,
}

impl Graph {
    /// Construct an empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a node. Returns the previous node with the same id, if any.
    pub fn add_node(&mut self, _node: Node) -> Option<Node> {
        unimplemented!("idl-graph: add_node — to be implemented after P0.1 schema lands")
    }

    /// Insert an edge. Both endpoints are expected to exist.
    pub fn add_edge(&mut self, _edge: Edge) -> Option<Edge> {
        unimplemented!("idl-graph: add_edge — to be implemented after P0.1 schema lands")
    }

    /// Look up a node by id.
    pub fn get_node(&self, _id: &NodeId) -> Option<&Node> {
        unimplemented!("idl-graph: get_node — to be implemented after P0.1 schema lands")
    }

    /// Iterate every node matching `kind`.
    pub fn query_by_kind<'a>(&'a self, _kind: &'a NodeKind) -> impl Iterator<Item = &'a Node> + 'a {
        // Placeholder: empty iterator until P0.1 lands.
        std::iter::empty()
    }

    /// Run the supplied constraints against the graph.
    ///
    /// Returns a [`Report`] on success or the collected [`ConstraintViolation`]s.
    /// P0.3 will provide the real implementation and a default constraint set.
    pub fn validate(
        &self,
        _constraints: &[Box<dyn Constraint>],
    ) -> Result<Report, Vec<ConstraintViolation>> {
        unimplemented!("idl-graph: validate — populated by P0.3 graph constraints")
    }
}
