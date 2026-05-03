//! In-memory property graph container.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::edge::{Edge, EdgeId};
use crate::kind::{EdgeKind, NodeKind};
use crate::node::{Node, NodeId};
use crate::state::NodeState;
use crate::validate::{Constraint, ValidationReport};

/// The IDL semantic property graph.
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
    pub fn add_node(&mut self, node: Node) -> Option<Node> {
        self.nodes.insert(node.id.clone(), node)
    }

    /// Insert an edge. Both endpoints are expected to exist; this is enforced
    /// at validation time, not at insertion time, so partial graphs can still
    /// be built incrementally.
    pub fn add_edge(&mut self, edge: Edge) -> Option<Edge> {
        self.edges.insert(edge.id.clone(), edge)
    }

    /// Look up a node by id.
    pub fn get_node(&self, id: &NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }

    /// Look up an edge by id.
    pub fn get_edge(&self, id: &EdgeId) -> Option<&Edge> {
        self.edges.get(id)
    }

    /// Iterate every node.
    pub fn iter_nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.values()
    }

    /// Iterate every edge.
    pub fn iter_edges(&self) -> impl Iterator<Item = &Edge> {
        self.edges.values()
    }

    /// Iterate every node matching `kind`.
    pub fn query_by_kind<'a>(&'a self, kind: &'a NodeKind) -> impl Iterator<Item = &'a Node> + 'a {
        self.nodes.values().filter(move |n| &n.kind == kind)
    }

    /// Iterate every node whose lifecycle state matches `state`.
    pub fn iter_nodes_by_state<'a>(
        &'a self,
        state: NodeState,
    ) -> impl Iterator<Item = &'a Node> + 'a {
        self.nodes.values().filter(move |n| n.state == state)
    }

    /// Iterate every edge originating at `from`.
    pub fn iter_edges_from<'a>(&'a self, from: &'a NodeId) -> impl Iterator<Item = &'a Edge> + 'a {
        self.edges.values().filter(move |e| &e.from == from)
    }

    /// Iterate every edge terminating at `to`.
    pub fn iter_edges_to<'a>(&'a self, to: &'a NodeId) -> impl Iterator<Item = &'a Edge> + 'a {
        self.edges.values().filter(move |e| &e.to == to)
    }

    /// Iterate every edge of a specific kind.
    pub fn iter_edges_by_kind<'a>(&'a self, kind: EdgeKind) -> impl Iterator<Item = &'a Edge> + 'a {
        self.edges.values().filter(move |e| e.kind == kind)
    }

    /// Run the supplied constraints against the graph and return a report
    /// grouping every violation by severity.
    pub fn validate(&self, constraints: &[Box<dyn Constraint>]) -> ValidationReport {
        let mut report = ValidationReport::default();
        for c in constraints {
            report.checked.push(c.name().to_string());
            for v in c.check(self) {
                report.push(v);
            }
        }
        report
    }
}
