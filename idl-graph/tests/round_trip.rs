//! Round-trip integration tests for `idl-graph`.
//!
//! These are placeholders that prove the crate compiles and integrates with
//! the workspace test harness. Real assertions land alongside P0.1 (schema)
//! and P0.3 (constraints).

use idl_graph::Graph;

#[test]
#[ignore = "TODO(P0.1): assert Graph -> JSON -> Graph round-trip via semantic-graph.schema.json"]
fn graph_json_round_trip() {
    let _g = Graph::new();
}

#[test]
#[ignore = "TODO(P0.3): assert default constraint set passes on a known-good fixture graph"]
fn default_constraints_pass_on_fixture() {
    let _g = Graph::new();
}
