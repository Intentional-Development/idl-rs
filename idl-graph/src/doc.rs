//! On-disk graph document (the JSON shape produced by extractors and
//! validated by `IDL/schemas/semantic-graph.schema.json` v0.1.0).
//!
//! The in-memory [`crate::Graph`] is an indexed, kernel-typed view used by
//! validators and constraints. The [`GraphDoc`] in this module is the
//! lossless, schema-shaped view used by tools that operate on extractor
//! output (drift detector, code emitters, etc.) without losing extension
//! props or unknown fields.
//!
//! The two views are intentionally decoupled: extractors round-trip
//! [`GraphDoc`]; the typed [`crate::Graph`] is reserved for validation
//! workflows where strict kernel typing matters.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Value as JsonValue;

/// Top-level graph document — matches `semantic-graph.schema.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphDoc {
    pub version: String,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub metadata: Value,
    #[serde(default)]
    pub nodes: Vec<NodeDoc>,
    #[serde(default)]
    pub edges: Vec<EdgeDoc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Value>,
}

/// Node document — `created_by`, `decision_refs`, `confidence`, free `props`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDoc {
    pub id: String,
    pub kind: String,
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    #[serde(default)]
    pub props: serde_json::Map<String, JsonValue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_anchors: Vec<SourceAnchorDoc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<ConfidenceDoc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decision_refs: Vec<String>,
}

/// Edge document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeDoc {
    pub id: String,
    pub kind: String,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub props: serde_json::Map<String, JsonValue>,
}

/// On-disk source anchor — `range` is the schema's object form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceAnchorDoc {
    pub uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<RangeDoc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

/// On-disk range (schema-shaped: line/byte/char are all optional).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RangeDoc {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_line: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_byte: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_byte: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_char: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_char: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceDoc {
    pub score: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
}

impl GraphDoc {
    /// Read a graph JSON file from disk.
    pub fn load(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)?;
        serde_json::from_str(&text)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// Iterate every node whose `kind` matches.
    pub fn nodes_of_kind<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a NodeDoc> + 'a {
        self.nodes.iter().filter(move |n| n.kind == kind)
    }

    /// Look up node by id.
    pub fn node(&self, id: &str) -> Option<&NodeDoc> {
        self.nodes.iter().find(|n| n.id == id)
    }
}
