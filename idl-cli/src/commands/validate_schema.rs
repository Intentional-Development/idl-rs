//! `idl validate-schema` — validates a JSON graph against
//! `IDL/schemas/semantic-graph.schema.json` (v0.1.0).

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;

const EMBEDDED_SCHEMA: &str = include_str!("../../../../IDL/schemas/semantic-graph.schema.json");

#[derive(Serialize)]
struct SchemaViolation {
    pointer: String,
    schema_pointer: String,
    message: String,
}

#[derive(Serialize)]
struct SchemaReport {
    schema_id: String,
    graph_path: String,
    valid: bool,
    violations: Vec<SchemaViolation>,
}

pub fn run(graph_path: PathBuf, schema_override: Option<PathBuf>, json: bool) -> Result<ExitCode> {
    let schema_text = match &schema_override {
        Some(p) => std::fs::read_to_string(p)
            .with_context(|| format!("read schema file {}", p.display()))?,
        None => EMBEDDED_SCHEMA.to_string(),
    };
    let schema_json: Value = serde_json::from_str(&schema_text).context("parse schema JSON")?;

    let graph_text = std::fs::read_to_string(&graph_path)
        .with_context(|| format!("read graph file {}", graph_path.display()))?;
    let graph_json: Value = serde_json::from_str(&graph_text).context("parse graph JSON")?;

    let compiled = jsonschema::JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .compile(&schema_json)
        .map_err(|e| anyhow::anyhow!("compile schema: {e}"))?;

    let mut violations: Vec<SchemaViolation> = Vec::new();
    if let Err(errors) = compiled.validate(&graph_json) {
        for err in errors {
            violations.push(SchemaViolation {
                pointer: err.instance_path.to_string(),
                schema_pointer: err.schema_path.to_string(),
                message: err.to_string(),
            });
        }
    }

    let schema_id = schema_json
        .get("$id")
        .and_then(|v| v.as_str())
        .unwrap_or("(no $id)")
        .to_string();

    let valid = violations.is_empty();
    let report = SchemaReport {
        schema_id,
        graph_path: graph_path.display().to_string(),
        valid,
        violations,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("idl validate-schema");
        println!("  schema: {}", report.schema_id);
        println!("  graph:  {}", report.graph_path);
        if valid {
            println!("  result: VALID");
        } else {
            println!("  result: INVALID ({} violations)", report.violations.len());
            for v in &report.violations {
                println!(
                    "    {} (schema: {}): {}",
                    if v.pointer.is_empty() {
                        "/"
                    } else {
                        &v.pointer
                    },
                    v.schema_pointer,
                    v.message
                );
            }
        }
    }

    Ok(ExitCode::from(if valid { 0 } else { 1 }))
}
