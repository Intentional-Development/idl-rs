//! TypeExpr AST node definitions (W17).

use serde::{Deserialize, Serialize};

/// Parsed TypeExpr AST node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeExpr {
    /// Primitive type: string, integer, number, boolean, object, unknown
    Primitive(PrimitiveType),
    /// Reference to a named DTO (e.g., "User", "LoginResponse")
    Reference(String),
    /// Array type (T[])
    Array(Box<TypeExpr>),
    /// Nullable type (T?)
    Nullable(Box<TypeExpr>),
    /// Union type (A|B|C)
    Union(Vec<TypeExpr>),
    /// Map type (Map<K, V>)
    Map {
        key: Box<TypeExpr>,
        value: Box<TypeExpr>,
    },
    /// Unit type (())
    Unit,
}

/// Primitive type literals
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrimitiveType {
    String,
    Integer,
    Number,
    Boolean,
    Object,
    Unknown,
}

impl PrimitiveType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PrimitiveType::String => "string",
            PrimitiveType::Integer => "integer",
            PrimitiveType::Number => "number",
            PrimitiveType::Boolean => "boolean",
            PrimitiveType::Object => "object",
            PrimitiveType::Unknown => "unknown",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "string" => Some(PrimitiveType::String),
            "integer" => Some(PrimitiveType::Integer),
            "number" => Some(PrimitiveType::Number),
            "boolean" => Some(PrimitiveType::Boolean),
            "object" => Some(PrimitiveType::Object),
            "unknown" => Some(PrimitiveType::Unknown),
            _ => None,
        }
    }
}
