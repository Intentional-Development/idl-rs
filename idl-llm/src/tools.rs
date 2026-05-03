//! Tool-definition catalog for the `idl interview` skill.
//!
//! Mirrors the five tools listed in `codex-runtime.md`. Schemas are intentionally
//! permissive (kernel-shape only); the CLI re-runs full graph-schema validation
//! on the assembled delta after each round.

use crate::ToolDef;
use serde_json::json;

const KERNEL_NODE_KINDS: &[&str] = &[
    "intent",
    "scope",
    "entity",
    "aggregate",
    "variant",
    "constraints",
    "event",
    "operation",
    "state_machine",
    "rule",
    "invariant",
    "policy",
    "api",
    "access_pattern",
    "mapping",
    "trace_link",
    "decision",
    "verification",
];

const KERNEL_EDGE_KINDS: &[&str] = &[
    "realizes",
    "verifies",
    "triggers",
    "emits",
    "handles",
    "constrains",
    "traces_to",
    "extracted_from",
    "supersedes",
    "decides",
    "implements",
    "belongs_to",
    "variant_of",
    "transitions",
    "queries",
    "authorizes",
    "contains",
    "derives_from",
];

pub fn default_tools() -> Vec<ToolDef> {
    vec![
        propose_node(),
        propose_edge(),
        ask_question(),
        record_decision(),
        query_existing_graph(),
    ]
}

fn propose_node() -> ToolDef {
    ToolDef {
        name: "propose_node".into(),
        description:
            "Append one schema-shaped proposed node to the round delta. Rejects non-kernel kinds \
             or nodes missing confidence/source_anchors."
                .into(),
        parameters: json!({
            "type": "object",
            "required": ["id", "kind", "state", "created_by", "confidence", "source_anchors"],
            "properties": {
                "id": { "type": "string" },
                "kind": { "type": "string", "enum": KERNEL_NODE_KINDS },
                "state": { "type": "string", "enum": ["proposed"] },
                "created_by": { "type": "string", "enum": ["ai", "human"] },
                "confidence": {
                    "type": "object",
                    "required": ["score", "model", "run_id"],
                    "properties": {
                        "score": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                        "model": { "type": "string" },
                        "run_id": { "type": "string" }
                    }
                },
                "source_anchors": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "required": ["uri"],
                        "properties": { "uri": { "type": "string" } }
                    }
                },
                "decision_refs": { "type": "array", "items": { "type": "string" } },
                "props": { "type": "object" }
            }
        }),
    }
}

fn propose_edge() -> ToolDef {
    ToolDef {
        name: "propose_edge".into(),
        description:
            "Append one kernel edge to the round delta. Rejects unknown endpoints unless they \
             are also proposed in the same round."
                .into(),
        parameters: json!({
            "type": "object",
            "required": ["id", "kind", "from", "to"],
            "properties": {
                "id": { "type": "string" },
                "kind": { "type": "string", "enum": KERNEL_EDGE_KINDS },
                "from": { "type": "string" },
                "to": { "type": "string" }
            }
        }),
    }
}

fn ask_question() -> ToolDef {
    ToolDef {
        name: "ask_question".into(),
        description:
            "Add one blocking question. Hard limit 3 per round; each question MUST include a \
             default, why_it_matters, and what it blocks."
                .into(),
        parameters: json!({
            "type": "object",
            "required": ["id", "question", "default", "why_it_matters", "blocks"],
            "properties": {
                "id": { "type": "string" },
                "question": { "type": "string" },
                "default": { "type": "string" },
                "why_it_matters": { "type": "string" },
                "blocks": { "type": "array", "items": { "type": "string" } }
            }
        }),
    }
}

fn record_decision() -> ToolDef {
    ToolDef {
        name: "record_decision".into(),
        description:
            "Create or update a proposed decision node and a lightweight ledger row tying it to \
             the answered question."
                .into(),
        parameters: json!({
            "type": "object",
            "required": ["id", "answer", "status"],
            "properties": {
                "id": { "type": "string" },
                "question_ref": { "type": ["string", "null"] },
                "answer": { "type": "string" },
                "status": { "type": "string", "enum": ["proposed", "accepted", "rejected"] }
            }
        }),
    }
}

fn query_existing_graph() -> ToolDef {
    ToolDef {
        name: "query_existing_graph".into(),
        description:
            "Read-only lookup against the accepted graph plus the accumulated session delta. \
             Returns a list of matching node summaries."
                .into(),
        parameters: json!({
            "type": "object",
            "required": ["query"],
            "properties": {
                "query": { "type": "string" },
                "kinds": {
                    "type": "array",
                    "items": { "type": "string", "enum": KERNEL_NODE_KINDS }
                }
            }
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_five_tools_with_unique_names() {
        let tools = default_tools();
        assert_eq!(tools.len(), 5);
        let mut names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        names.sort();
        assert_eq!(
            names,
            vec![
                "ask_question",
                "propose_edge",
                "propose_node",
                "query_existing_graph",
                "record_decision",
            ]
        );
    }
}
