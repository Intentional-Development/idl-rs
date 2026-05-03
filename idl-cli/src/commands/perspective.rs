//! `idl perspective` — project a graph through a role-based filter.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use idl_graph::GraphDoc;
use serde::Deserialize;

const DEFAULT_PERSPECTIVES_TOML: &str = r#"
[meta]
version = "0.1.0"
description = "IDL role-based perspective definitions"

[roles.product-manager]
description = "Strategic view: intents, scope boundaries, key decisions"
node_kinds = ["intent", "scope", "decision"]
edge_kinds = ["contains", "derives_from", "decides", "realizes"]
include_props = true

[roles.frontend-developer]
description = "Consumer view: DTOs, APIs, variants, operations"
node_kinds = ["entity", "variant", "api", "operation"]
edge_kinds = ["implements", "variant_of", "queries", "triggers", "realizes"]
include_props = true

[roles.backend-developer]
description = "Service view: domain model, behavior, events, rules"
node_kinds = ["entity", "aggregate", "operation", "event", "state_machine", "rule", "invariant"]
edge_kinds = ["belongs_to", "emits", "handles", "triggers", "transitions", "constrains", "realizes"]
include_props = true

[roles.dba]
description = "Data view: persistence model, access paths, constraints"
node_kinds = ["entity", "aggregate", "access_pattern", "constraints"]
edge_kinds = ["belongs_to", "queries", "constrains", "contains"]
include_props = true

[roles.devops]
description = "Delivery view: mappings, traces, operational surface"
node_kinds = ["mapping", "trace_link", "operation", "api"]
edge_kinds = ["traces_to", "extracted_from", "implements", "triggers", "handles"]
include_props = true

[roles.sre]
description = "Reliability view: events, state, operations, invariants"
node_kinds = ["event", "state_machine", "operation", "invariant"]
edge_kinds = ["emits", "handles", "triggers", "transitions", "constrains"]
include_props = true

[roles.security]
description = "Authorization and compliance view: policies, constraints, invariants, APIs"
node_kinds = ["policy", "constraints", "invariant", "api", "operation"]
edge_kinds = ["authorizes", "constrains", "implements", "contains"]
include_props = true

[roles.qa]
description = "Testability view: verifications, invariants, operations, events"
node_kinds = ["verification", "invariant", "operation", "event"]
edge_kinds = ["verifies", "constrains", "emits", "handles", "triggers"]
include_props = true

[roles.technical-writer]
description = "Documentation surface: intent, scope, domain nouns, APIs, decisions"
node_kinds = ["intent", "scope", "entity", "api", "decision"]
edge_kinds = ["contains", "realizes", "decides", "implements", "derives_from"]
include_props = true
"#;

#[derive(Debug, Deserialize)]
struct PerspectivesConfig {
    roles: BTreeMap<String, RoleConfig>,
}

#[derive(Debug, Deserialize)]
struct RoleConfig {
    description: String,
    node_kinds: Vec<String>,
    edge_kinds: Vec<String>,
    #[serde(default = "default_true")]
    include_props: bool,
}

fn default_true() -> bool {
    true
}

pub fn run(
    role: String,
    graph_path: PathBuf,
    config_path: Option<PathBuf>,
    json: bool,
) -> Result<ExitCode> {
    let graph = GraphDoc::load(&graph_path)
        .with_context(|| format!("load graph {}", graph_path.display()))?;
    let config_text = match config_path {
        Some(path) => std::fs::read_to_string(&path)
            .with_context(|| format!("read perspectives config {}", path.display()))?,
        None => DEFAULT_PERSPECTIVES_TOML.to_string(),
    };
    let config: PerspectivesConfig =
        toml::from_str(&config_text).context("parse perspectives TOML")?;
    let role_config = config.roles.get(&role).ok_or_else(|| {
        anyhow::anyhow!(
            "unknown perspective role `{}`; available: {}",
            role,
            config.roles.keys().cloned().collect::<Vec<_>>().join(", ")
        )
    })?;

    let projected = project_graph(graph, role_config);
    if json {
        println!("{}", serde_json::to_string_pretty(&projected)?);
    } else {
        print_markdown(&role, role_config, &projected);
    }

    Ok(ExitCode::SUCCESS)
}

fn project_graph(mut graph: GraphDoc, role: &RoleConfig) -> GraphDoc {
    let node_kinds: BTreeSet<&str> = role.node_kinds.iter().map(String::as_str).collect();
    let edge_kinds: BTreeSet<&str> = role.edge_kinds.iter().map(String::as_str).collect();

    graph
        .nodes
        .retain(|node| node_kinds.contains(node.kind.as_str()));
    if !role.include_props {
        for node in &mut graph.nodes {
            node.props.clear();
        }
    }

    let surviving_ids: BTreeSet<&str> = graph.nodes.iter().map(|node| node.id.as_str()).collect();
    graph.edges.retain(|edge| {
        edge_kinds.contains(edge.kind.as_str())
            && surviving_ids.contains(edge.from.as_str())
            && surviving_ids.contains(edge.to.as_str())
    });
    if !role.include_props {
        for edge in &mut graph.edges {
            edge.props.clear();
        }
    }

    graph
}

fn print_markdown(role: &str, role_config: &RoleConfig, graph: &GraphDoc) {
    println!("# IDL Perspective: {role}");
    println!();
    println!("{}", role_config.description);
    println!();
    println!("## Nodes ({})", graph.nodes.len());
    for node in &graph.nodes {
        let name = node
            .props
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&node.id);
        println!("- `{}` ({}) — {}", node.id, node.kind, name);
    }
    println!();
    println!("## Edges ({})", graph.edges.len());
    for edge in &graph.edges {
        println!(
            "- `{}`: `{}` -{}-> `{}`",
            edge.id, edge.from, edge.kind, edge.to
        );
    }
}
