//! Per-round delta validation.
//!
//! Two layers:
//!   1. A kernel-kind sanity pass that uses [`idl_graph::NodeKind`] /
//!      [`idl_graph::EdgeKind`] to reject any non-kernel `kind` value.
//!   2. The full semantic-graph JSON Schema (`IDL/schemas/semantic-graph.schema.json`)
//!      compiled once and re-used across rounds.

use anyhow::Result;
use idl_graph::{EdgeKind, NodeKind};
use jsonschema::JSONSchema;
use serde_json::Value;
use std::str::FromStr;

const EMBEDDED_SCHEMA: &str =
    include_str!("../../../IDL/schemas/semantic-graph.schema.json");

#[derive(Debug, thiserror::Error)]
#[error("invalid round delta: {0}")]
pub struct ValidationError(pub String);

pub struct DeltaValidator {
    schema: JSONSchema,
}

impl DeltaValidator {
    pub fn new() -> Result<Self> {
        let schema_json: Value = serde_json::from_str(EMBEDDED_SCHEMA)?;
        let schema = JSONSchema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .compile(&schema_json)
            .map_err(|e| anyhow::anyhow!("compile semantic-graph schema: {e}"))?;
        Ok(Self { schema })
    }

    /// Validate a single round's `graph_delta` (already shaped as
    /// `{version, nodes, edges}`).
    pub fn validate(&self, delta: &Value) -> Result<(), ValidationError> {
        // 1. Kernel-kind pass with crisp error messages.
        let nodes = delta.get("nodes").and_then(Value::as_array);
        if nodes.is_none() {
            return Err(ValidationError("graph_delta.nodes must be an array".into()));
        }
        for n in nodes.unwrap() {
            let kind = n
                .get("kind")
                .and_then(Value::as_str)
                .ok_or_else(|| ValidationError("node missing `kind`".into()))?;
            NodeKind::from_str(kind).map_err(|_| {
                ValidationError(format!(
                    "non-kernel node kind `{kind}` (allowed: 18 kernel kinds)"
                ))
            })?;
            if n.get("id").and_then(Value::as_str).is_none() {
                return Err(ValidationError("node missing `id`".into()));
            }
            if n.get("confidence").is_none() {
                return Err(ValidationError(format!(
                    "node `{}` missing required `confidence`",
                    n.get("id").and_then(Value::as_str).unwrap_or("?")
                )));
            }
        }
        if let Some(edges) = delta.get("edges").and_then(Value::as_array) {
            for e in edges {
                let kind = e
                    .get("kind")
                    .and_then(Value::as_str)
                    .ok_or_else(|| ValidationError("edge missing `kind`".into()))?;
                EdgeKind::from_str(kind).map_err(|_| {
                    ValidationError(format!("non-kernel edge kind `{kind}`"))
                })?;
            }
        }
        // 2. Full schema pass.
        if let Err(errors) = self.schema.validate(delta) {
            let summary: Vec<String> = errors
                .take(5)
                .map(|err| format!("{} at {}", err, err.instance_path))
                .collect();
            return Err(ValidationError(format!(
                "schema violations: [{}]",
                summary.join("; ")
            )));
        }
        Ok(())
    }
}

/// Wrap a per-round `graph_delta` (which may omit the top-level `version`)
/// into a full graph document that the schema can validate.
pub fn delta_to_graph_doc(delta: &Value) -> Value {
    let mut out = serde_json::Map::new();
    out.insert(
        "version".into(),
        delta.get("version").cloned().unwrap_or_else(|| Value::String("0.1.0".into())),
    );
    out.insert(
        "nodes".into(),
        delta.get("nodes").cloned().unwrap_or_else(|| Value::Array(vec![])),
    );
    out.insert(
        "edges".into(),
        delta.get("edges").cloned().unwrap_or_else(|| Value::Array(vec![])),
    );
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_non_kernel_kind() {
        let v = DeltaValidator::new().unwrap();
        let delta = json!({"version":"0.1.0","nodes":[{"id":"x:1","kind":"made_up","confidence":{"score":0.5,"model":"m","run_id":"r"}}],"edges":[]});
        assert!(v.validate(&delta).is_err());
    }

    #[test]
    fn accepts_demo_round_1() {
        let v = DeltaValidator::new().unwrap();
        let raw = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/demo-todo-app/round-1.json"
        ))
        .unwrap();
        let resp: Value = serde_json::from_str(&raw).unwrap();
        let delta = delta_to_graph_doc(&resp["graph_delta"]);
        v.validate(&delta).unwrap();
    }
}
