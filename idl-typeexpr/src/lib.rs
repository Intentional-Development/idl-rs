//! TypeExpr DSL — Derived Layer (W17).
//!
//! Bidirectional compiler between canonical `kind` discriminator forms and
//! human-readable type expressions. TypeExpr is a **derived projection** —
//! it does NOT replace the canonical schema but provides diagnostic and
//! documentation-friendly syntax.
//!
//! ## Public API
//!
//! - [`parse_type_expr`] — Parse a TypeExpr string into an AST
//! - [`render_type_expr`] — Render a canonical DTO as a TypeExpr string
//! - [`expr_to_kind`] — Compile a TypeExpr AST into a canonical DTO skeleton
//!
//! ## Grammar (EBNF)
//!
//! ```text
//! TypeExpr     = UnionExpr ;
//! UnionExpr    = NullableExpr ( "|" NullableExpr )* ;
//! NullableExpr = ArrayExpr [ "?" ] ;
//! ArrayExpr    = AtomExpr ( "[]" )* ;
//! AtomExpr     = MapExpr | GroupExpr | Reference | Primitive | UnitLit ;
//!
//! MapExpr      = "Map" "<" TypeExpr "," TypeExpr ">" ;
//! GroupExpr    = "(" TypeExpr ")" ;
//! Reference    = UpperIdent ( "." UpperIdent )* ;
//! Primitive    = "string" | "integer" | "number" | "boolean" | "object" | "unknown" ;
//! UnitLit      = "()" ;
//! ```
//!
//! ## Round-trip guarantee
//!
//! For all v0.1.6 kinds: `expr_to_kind(name, render_type_expr(dto))` produces
//! a DTO skeleton that matches the original DTO's type shape. TypeExpr carries
//! **type shape only** — identity, provenance, and discriminator metadata stay
//! in canonical form.

pub mod parser;
pub mod render;
pub mod types;

use std::collections::BTreeMap;

pub use parser::{parse_type_expr, ParseError};
pub use render::{render_expr_ast, render_field_type_expr, render_type_expr};
pub use types::{PrimitiveType, TypeExpr};

use idl_graph::extensions_dto::{DtoDefinition, DtoKind, DtoVariant};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum CompileError {
    #[error("bare primitives cannot be standalone DTOs")]
    BarePrimitive,
    #[error("invalid DTO kind for expression")]
    InvalidKind,
}

/// Compile a TypeExpr AST into a canonical DtoDefinition skeleton.
///
/// This produces a minimal DTO with the correct `kind` and shape fields.
/// Metadata fields (`state`, `created_by`, etc.) are populated with defaults
/// and should be overwritten by the caller.
///
/// # Discriminator metadata loss
///
/// TypeExpr cannot represent `discriminator` objects. When compiling a union
/// expression back to canonical form, the discriminator field is omitted.
/// This is acceptable per W17 design — discriminator metadata is preserved
/// only in canonical form.
pub fn expr_to_kind(expr: &TypeExpr, name: &str) -> Result<DtoDefinition, CompileError> {
    let id = if name.starts_with("dto:") {
        name.to_string()
    } else {
        format!("dto:{}", name)
    };

    let (kind, nullable, dto_fields) = analyze_expr(expr)?;

    Ok(DtoDefinition {
        id,
        kind,
        nullable,
        state: "draft".to_string(),
        created_by: "typeexpr".to_string(),
        base: dto_fields.base,
        values: dto_fields.values,
        value_type: dto_fields.value_type,
        key_type: dto_fields.key_type,
        items: dto_fields.items,
        variants: dto_fields.variants,
        discriminator: None, // Discriminator metadata is NOT preserved in TypeExpr
        cursor_field: None,
        has_more_field: None,
        total_field: None,
        meta_fields: None,
        wrapper: false,
        wraps: None,
        pick: None,
        omit: None,
        required: vec![],
        extras: BTreeMap::new(),
        source_anchors: vec![],
        decision_refs: vec![],
        confidence: None,
    })
}

struct DtoFields {
    base: Option<String>,
    values: Option<Vec<String>>,
    value_type: Option<String>,
    key_type: Option<String>,
    items: Option<String>,
    variants: Option<Vec<DtoVariant>>,
}

impl DtoFields {
    fn empty() -> Self {
        Self {
            base: None,
            values: None,
            value_type: None,
            key_type: None,
            items: None,
            variants: None,
        }
    }
}

fn analyze_expr(expr: &TypeExpr) -> Result<(DtoKind, bool, DtoFields), CompileError> {
    match expr {
        TypeExpr::Primitive(_) => Err(CompileError::BarePrimitive),
        
        TypeExpr::Reference(_name) => {
            // Reference to a named DTO — assume object kind (could be enum,
            // but that requires graph lookup which is outside TypeExpr's scope)
            Ok((DtoKind::Object, false, DtoFields::empty()))
        }
        
        TypeExpr::Array(inner) => {
            let items_ref = match inner.as_ref() {
                TypeExpr::Reference(name) => format!("dto:{}", name),
                TypeExpr::Primitive(p) => p.as_str().to_string(),
                _ => return Err(CompileError::InvalidKind),
            };
            
            let mut fields = DtoFields::empty();
            fields.items = Some(items_ref);
            Ok((DtoKind::ArrayAlias, false, fields))
        }
        
        TypeExpr::Nullable(inner) => {
            let (kind, _inner_nullable, fields) = analyze_expr(inner)?;
            Ok((kind, true, fields))
        }
        
        TypeExpr::Union(variants) => {
            let dto_variants: Vec<DtoVariant> = variants
                .iter()
                .map(expr_to_variant)
                .collect::<Result<_, _>>()?;
            
            let mut fields = DtoFields::empty();
            fields.variants = Some(dto_variants);
            Ok((DtoKind::Union, false, fields))
        }
        
        TypeExpr::Map { key, value } => {
            let key_str = match key.as_ref() {
                TypeExpr::Primitive(p) => p.as_str().to_string(),
                TypeExpr::Reference(name) => name.clone(),
                _ => return Err(CompileError::InvalidKind),
            };
            
            let value_str = match value.as_ref() {
                TypeExpr::Primitive(p) => p.as_str().to_string(),
                TypeExpr::Reference(name) => format!("dto:{}", name),
                _ => return Err(CompileError::InvalidKind),
            };
            
            let mut fields = DtoFields::empty();
            fields.key_type = Some(key_str);
            fields.value_type = Some(value_str);
            Ok((DtoKind::Map, false, fields))
        }
        
        TypeExpr::Unit => Ok((DtoKind::Unit, false, DtoFields::empty())),
    }
}

fn expr_to_variant(expr: &TypeExpr) -> Result<DtoVariant, CompileError> {
    match expr {
        TypeExpr::Primitive(p) => Ok(DtoVariant {
            ty: Some(p.as_str().to_string()),
            ref_: None,
            array: false,
        }),
        
        TypeExpr::Reference(name) => Ok(DtoVariant {
            ty: None,
            ref_: Some(format!("dto:{}", name)),
            array: false,
        }),
        
        TypeExpr::Array(inner) => {
            match inner.as_ref() {
                TypeExpr::Primitive(p) => Ok(DtoVariant {
                    ty: Some(p.as_str().to_string()),
                    ref_: None,
                    array: true,
                }),
                TypeExpr::Reference(name) => Ok(DtoVariant {
                    ty: None,
                    ref_: Some(format!("dto:{}", name)),
                    array: true,
                }),
                _ => Err(CompileError::InvalidKind),
            }
        }
        
        _ => Err(CompileError::InvalidKind),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expr_to_kind_object() {
        let expr = TypeExpr::Reference("LoginUser".to_string());
        let dto = expr_to_kind(&expr, "LoginUser").unwrap();
        assert_eq!(dto.id, "dto:LoginUser");
        assert_eq!(dto.kind, DtoKind::Object);
        assert!(!dto.nullable);
    }

    #[test]
    fn test_expr_to_kind_array() {
        let expr = TypeExpr::Array(Box::new(TypeExpr::Reference("Bill".to_string())));
        let dto = expr_to_kind(&expr, "BillArray").unwrap();
        assert_eq!(dto.kind, DtoKind::ArrayAlias);
        assert_eq!(dto.items, Some("dto:Bill".to_string()));
    }

    #[test]
    fn test_expr_to_kind_union() {
        let expr = TypeExpr::Union(vec![
            TypeExpr::Reference("TransactionSplit".to_string()),
            TypeExpr::Reference("TransactionDefault".to_string()),
        ]);
        let dto = expr_to_kind(&expr, "TransactionRead").unwrap();
        assert_eq!(dto.kind, DtoKind::Union);
        assert_eq!(dto.variants.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_expr_to_kind_map() {
        let expr = TypeExpr::Map {
            key: Box::new(TypeExpr::Primitive(PrimitiveType::String)),
            value: Box::new(TypeExpr::Primitive(PrimitiveType::String)),
        };
        let dto = expr_to_kind(&expr, "PrepareUploadResponse").unwrap();
        assert_eq!(dto.kind, DtoKind::Map);
        assert_eq!(dto.key_type, Some("string".to_string()));
        assert_eq!(dto.value_type, Some("string".to_string()));
    }

    #[test]
    fn test_expr_to_kind_nullable() {
        let expr = TypeExpr::Nullable(Box::new(TypeExpr::Map {
            key: Box::new(TypeExpr::Primitive(PrimitiveType::String)),
            value: Box::new(TypeExpr::Reference("Entry".to_string())),
        }));
        let dto = expr_to_kind(&expr, "NullableMap").unwrap();
        assert_eq!(dto.kind, DtoKind::Map);
        assert!(dto.nullable);
    }
}
