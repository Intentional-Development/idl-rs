//! Lift a parsed [`idl_core::IdlDocument`] into a kernel-aware
//! [`idl_graph::Graph`], collecting a [`SemanticLossReport`] for blocks the
//! kernel does not represent.
//!
//! This is intentionally small: the goal is `idl validate` over the kernel
//! surface, not a complete IDL→graph compiler. Block kinds that are not
//! kernel constructs (e.g. `infrastructure`, `service`, `ux_flow`) are
//! recorded as `unknown_construct` losses so the report stays honest.

use std::collections::BTreeMap;

use idl_core::{Block, IdlDocument};
use idl_graph::{
    Graph, LossEntry, LossReason, Node, NodeId, NodeKind, NodeState, SemanticLossReport,
};

pub struct LiftResult {
    pub graph: Graph,
    pub loss: SemanticLossReport,
}

pub fn lift_document(doc: &IdlDocument, source_path: &str) -> LiftResult {
    let mut graph = Graph::new();
    let mut loss = SemanticLossReport::new(source_path);
    loss.total_blocks = doc.blocks.len();

    for block in &doc.blocks {
        if let Some((kind, name)) = block_to_kernel(block) {
            let id = NodeId(format!("{}::{}", kind.canonical(), name));
            let node = Node {
                id: id.clone(),
                kind,
                state: NodeState::Accepted,
                props: BTreeMap::new(),
                source_anchors: vec![idl_graph::SourceAnchor {
                    uri: source_path.to_string(),
                    range: None,
                    hash: None,
                }],
                confidence: None,
            };
            graph.add_node(node);
            loss.recognized_blocks += 1;
        } else {
            loss.lost_blocks.push(LossEntry {
                block_kind: block_label(block).to_string(),
                line_range: (0, 0),
                reason: LossReason::UnknownConstruct,
                raw_excerpt: block_label(block).to_string(),
            });
        }
    }

    LiftResult { graph, loss }
}

fn block_to_kernel(block: &Block) -> Option<(NodeKind, String)> {
    let pair = match block {
        Block::Intent(b) => (NodeKind::Intent, b.name.clone()),
        Block::Scope(b) => (NodeKind::Scope, b.name.clone()),
        Block::Entity(b) => (NodeKind::Entity, b.name.clone()),
        Block::Aggregate(b) => (NodeKind::Aggregate, b.name.clone()),
        Block::Variant(b) => (NodeKind::Variant, b.name.clone()),
        Block::Constraints(b) => (NodeKind::Constraints, b.name.clone()),
        Block::Event(b) => (NodeKind::Event, b.name.clone()),
        Block::Operation(b) => (NodeKind::Operation, b.name.clone()),
        Block::StateMachine(b) => (NodeKind::StateMachine, b.name.clone()),
        Block::Rule(b) => (NodeKind::Rule, b.name.clone()),
        Block::Invariant(b) => (NodeKind::Invariant, b.name.clone()),
        Block::Policy(b) => (NodeKind::Policy, b.name.clone()),
        Block::Api(b) => (NodeKind::Api, b.name.clone()),
        Block::Mapping(b) => (NodeKind::Mapping, b.name.clone()),
        Block::TraceLink(b) => (NodeKind::TraceLink, format!("{}__{}", b.from, b.to)),
        Block::Decision(b) => (NodeKind::Decision, b.name.clone()),
        Block::Verification(b) => (NodeKind::Verification, b.name.clone()),
        // Out-of-kernel today.
        _ => return None,
    };
    Some(pair)
}

fn block_label(block: &Block) -> &'static str {
    match block {
        Block::Intent(_) => "intent",
        Block::Scope(_) => "scope",
        Block::Entity(_) => "entity",
        Block::Aggregate(_) => "aggregate",
        Block::Variant(_) => "variant",
        Block::Constraints(_) => "constraints",
        Block::Event(_) => "event",
        Block::Operation(_) => "operation",
        Block::StateMachine(_) => "state_machine",
        Block::Rule(_) => "rule",
        Block::Invariant(_) => "invariant",
        Block::Policy(_) => "policy",
        Block::Api(_) => "api",
        Block::Mapping(_) => "mapping",
        Block::TraceLink(_) => "trace_link",
        Block::Decision(_) => "decision",
        Block::Verification(_) => "verification",
        Block::Service(_) => "service",
        Block::Infrastructure(_) => "infrastructure",
        Block::Requires(_) => "requires",
        Block::UxFlow(_) => "ux_flow",
        Block::UxComponent(_) => "ux_component",
        Block::Pattern(_) => "pattern",
        Block::Execution(_) => "execution",
        Block::Localization(_) => "localization",
        Block::Profile(_) => "profile",
        Block::Job(_) => "job",
        Block::Dependency(_) => "dependency",
        Block::Extension(_) => "extension",
    }
}
