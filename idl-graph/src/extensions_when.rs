//! Extension `edge.props.when` — Wave 15 (structured conditional execution).
//!
//! The `when` field on `edge.props` declares conditional execution semantics
//! for edges, enabling faithful round-trip of branching logic (IF nodes,
//! Switch nodes, conditional routing) from workflow platforms like n8n.
//!
//! This module owns:
//!   1. The `When` type (parsed from edge.props.when in the JSON graph).
//!   2. Backward compatibility with string-form when.
//!   3. Optional validation helpers for consistency checks (ast/vars vs expr).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Structured conditional execution expression.
///
/// Can be either:
/// - A plain string (legacy form, backward compatible)
/// - A structured object with `expr` (canonical), optional `ast`, `vars`, `lang`
///
/// The `expr` field is the source-of-truth; `ast` and `vars` are derived metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum When {
    /// Legacy string-form when expression.
    /// Example: "prev_node.status == 'success'"
    String(String),
    
    /// Structured when expression (v0.1.4+).
    Structured(WhenStructured),
}

/// Structured when expression with optional AST and variable tracking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WhenStructured {
    /// Canonical expression string (source-of-truth).
    /// Example: "$json.destination === 'London'"
    pub expr: String,
    
    /// Optional structured AST representation.
    /// Shape depends on the expression language (CEL, jsonlogic, n8n, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ast: Option<Value>,
    
    /// Optional list of variables/dependencies read by the expression.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vars: Vec<WhenVar>,
    
    /// Optional expression language identifier.
    /// Examples: "n8n", "cel", "jsonlogic", "jq"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
}

/// A variable reference within a when expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WhenVar {
    /// Variable name as it appears in the expression.
    /// Example: "$json.destination"
    pub name: String,
    
    /// Optional source node or context for this variable.
    /// Example: "node:fetch-packages", "prev_node", "workflow_data"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

impl When {
    /// Get the canonical expression string from either form.
    pub fn expr(&self) -> &str {
        match self {
            When::String(s) => s,
            When::Structured(w) => &w.expr,
        }
    }
    
    /// Check if this is the structured form.
    pub fn is_structured(&self) -> bool {
        matches!(self, When::Structured(_))
    }
    
    /// Get the structured form if present, None for string form.
    pub fn structured(&self) -> Option<&WhenStructured> {
        match self {
            When::Structured(w) => Some(w),
            When::String(_) => None,
        }
    }
}

impl WhenStructured {
    /// Create a new structured when expression with just the expr field.
    pub fn new(expr: impl Into<String>) -> Self {
        Self {
            expr: expr.into(),
            ast: None,
            vars: Vec::new(),
            lang: None,
        }
    }
    
    /// Builder method to add AST.
    pub fn with_ast(mut self, ast: Value) -> Self {
        self.ast = Some(ast);
        self
    }
    
    /// Builder method to add a variable.
    pub fn with_var(mut self, name: impl Into<String>, source: Option<String>) -> Self {
        self.vars.push(WhenVar {
            name: name.into(),
            source,
        });
        self
    }
    
    /// Builder method to set the language.
    pub fn with_lang(mut self, lang: impl Into<String>) -> Self {
        self.lang = Some(lang.into());
        self
    }
}

impl WhenVar {
    /// Create a new variable reference.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: None,
        }
    }
    
    /// Create a variable reference with a source.
    pub fn with_source(name: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: Some(source.into()),
        }
    }
}

/// Parse `edge.props.when` from a Value (if present).
pub fn parse_when(props: &BTreeMap<String, Value>) -> Option<When> {
    props
        .get("when")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_string_form() {
        let json = json!("prev_node.status == 'success'");
        let when: When = serde_json::from_value(json).unwrap();
        assert_eq!(when.expr(), "prev_node.status == 'success'");
        assert!(!when.is_structured());
    }

    #[test]
    fn test_parse_structured_minimal() {
        let json = json!({
            "expr": "$json.destination === 'London'"
        });
        let when: When = serde_json::from_value(json).unwrap();
        assert_eq!(when.expr(), "$json.destination === 'London'");
        assert!(when.is_structured());
    }

    #[test]
    fn test_parse_structured_full() {
        let json = json!({
            "expr": "$json.weight > 10",
            "ast": {
                "op": ">",
                "left": {"var": "$json.weight"},
                "right": {"literal": 10}
            },
            "vars": [
                {"name": "$json.weight", "source": "node:fetch-packages"}
            ],
            "lang": "n8n"
        });
        let when: When = serde_json::from_value(json).unwrap();
        assert_eq!(when.expr(), "$json.weight > 10");
        let structured = when.structured().unwrap();
        assert!(structured.ast.is_some());
        assert_eq!(structured.vars.len(), 1);
        assert_eq!(structured.vars[0].name, "$json.weight");
        assert_eq!(structured.lang.as_deref(), Some("n8n"));
    }

    #[test]
    fn test_builder() {
        let when = WhenStructured::new("$json.x > 5")
            .with_var("$json.x", Some("node:src".into()))
            .with_lang("n8n");
        
        assert_eq!(when.expr, "$json.x > 5");
        assert_eq!(when.vars.len(), 1);
        assert_eq!(when.lang.as_deref(), Some("n8n"));
    }

    #[test]
    fn test_roundtrip_string() {
        let when = When::String("test == true".into());
        let json = serde_json::to_value(&when).unwrap();
        let parsed: When = serde_json::from_value(json).unwrap();
        assert_eq!(when, parsed);
    }

    #[test]
    fn test_roundtrip_structured() {
        let when = When::Structured(
            WhenStructured::new("$json.x > 5")
                .with_var("$json.x", None)
                .with_lang("n8n"),
        );
        let json = serde_json::to_value(&when).unwrap();
        let parsed: When = serde_json::from_value(json).unwrap();
        assert_eq!(when, parsed);
    }
}
