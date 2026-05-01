//! Round-trip + constraint integration tests for `idl-graph`.

use std::collections::BTreeMap;

use idl_graph::{
    default_constraints, Confidence, Edge, EdgeId, EdgeKind, Graph, LossEntry, LossReason, Node,
    NodeId, NodeKind, NodeState, SemanticLossReport, SourceAnchor,
};

fn anchor(uri: &str) -> SourceAnchor {
    SourceAnchor {
        uri: uri.into(),
        range: Some((0, 1)),
        hash: None,
    }
}

fn node(id: &str, kind: NodeKind, state: NodeState) -> Node {
    Node {
        id: NodeId(id.into()),
        kind,
        state,
        props: BTreeMap::new(),
        source_anchors: vec![anchor("idl://fixture")],
        confidence: None,
    }
}

fn edge(id: &str, kind: EdgeKind, from: &str, to: &str) -> Edge {
    Edge {
        id: EdgeId(id.into()),
        kind,
        from: NodeId(from.into()),
        to: NodeId(to.into()),
        props: BTreeMap::new(),
    }
}

#[test]
fn graph_json_round_trip() {
    let mut g = Graph::new();
    g.add_node(node("intent-1", NodeKind::Intent, NodeState::Accepted));
    g.add_node(node("api-1", NodeKind::Api, NodeState::Accepted));
    g.add_edge(edge("e1", EdgeKind::Realizes, "api-1", "intent-1"));

    let json = serde_json::to_string(&g).expect("serialize");
    let back: Graph = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(back.nodes.len(), 2);
    assert_eq!(back.edges.len(), 1);
    assert_eq!(
        back.get_node(&NodeId("intent-1".into())).unwrap().kind,
        NodeKind::Intent
    );
}

#[test]
fn default_constraints_pass_on_fixture() {
    let mut g = Graph::new();
    g.add_node(node("intent-1", NodeKind::Intent, NodeState::Accepted));
    g.add_node(node("api-1", NodeKind::Api, NodeState::Accepted));
    g.add_edge(edge("e1", EdgeKind::Realizes, "api-1", "intent-1"));

    let report = g.validate(&default_constraints());
    assert!(
        report.ok(),
        "expected no errors, got {:?}",
        report.errors
    );
    assert_eq!(report.checked.len(), 6);
}

#[test]
fn accepted_node_without_provenance_errors() {
    let mut g = Graph::new();
    let mut n = node("rule-1", NodeKind::Rule, NodeState::Accepted);
    n.source_anchors.clear();
    g.add_node(n);

    let report = g.validate(&default_constraints());
    assert!(!report.ok());
    assert!(report
        .errors
        .iter()
        .any(|v| v.rule == "accepted-nodes-have-provenance"));
}

#[test]
fn proposed_node_requires_confidence() {
    let mut g = Graph::new();
    g.add_node(node("entity-1", NodeKind::Entity, NodeState::Proposed));

    let report = g.validate(&default_constraints());
    assert!(report
        .errors
        .iter()
        .any(|v| v.rule == "proposed-nodes-have-confidence"));

    // adding a confidence record clears the error
    let mut g2 = Graph::new();
    let mut n = node("entity-2", NodeKind::Entity, NodeState::Proposed);
    n.confidence = Some(Confidence {
        score: 0.42,
        model: "claude-opus-4.7".into(),
        run_id: "run-xyz".into(),
    });
    g2.add_node(n);
    let r2 = g2.validate(&default_constraints());
    assert!(r2
        .errors
        .iter()
        .all(|v| v.rule != "proposed-nodes-have-confidence"));
}

#[test]
fn dangling_edge_endpoint_errors() {
    let mut g = Graph::new();
    g.add_node(node("api-1", NodeKind::Api, NodeState::Accepted));
    g.add_edge(edge("e-bad", EdgeKind::Realizes, "api-1", "ghost"));

    let report = g.validate(&default_constraints());
    assert!(report
        .errors
        .iter()
        .any(|v| v.rule == "edge-endpoints-exist"));
}

#[test]
fn loss_report_render_and_coverage() {
    let mut r = SemanticLossReport::new("examples/firefly.idl");
    r.total_blocks = 4;
    r.recognized_blocks = 3;
    r.lost_blocks.push(LossEntry {
        block_kind: "job".into(),
        line_range: (12, 18),
        reason: LossReason::ExtensionNotEnabled,
        raw_excerpt: "job DailyReconcile { schedule: cron(\"0 3 * * *\") }".into(),
    });

    assert!((r.coverage_pct() - 75.0).abs() < 1e-3);
    let md = r.render_markdown();
    assert!(md.contains("examples/firefly.idl"));
    assert!(md.contains("75.0%"));
    assert!(md.contains("extension_not_enabled"));
    assert!(md.contains("job"));
}
