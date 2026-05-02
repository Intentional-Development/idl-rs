//! Render canonical DTOs as TypeExpr strings (W17).

use crate::types::TypeExpr;
use idl_graph::extensions_dto::{DtoDefinition, DtoExtra, DtoKind, DtoVariant};

pub fn render_type_expr(dto: &DtoDefinition) -> String {
    let base = match dto.kind {
        DtoKind::Object | DtoKind::Enum => {
            extract_name(&dto.id)
        }
        DtoKind::Map => {
            let key = dto.key_type.as_deref().unwrap_or("string");
            let value = dto.value_type.as_deref().unwrap_or("string");
            let value_expr = if value.starts_with("dto:") {
                extract_name(value)
            } else {
                value.to_string()
            };
            format!("Map<{}, {}>", key, value_expr)
        }
        DtoKind::Unit => "()".to_string(),
        DtoKind::ArrayAlias => {
            if let Some(items) = &dto.items {
                let item_expr = if items.starts_with("dto:") {
                    extract_name(items)
                } else {
                    items.to_string()
                };
                format!("{}[]", item_expr)
            } else {
                "unknown[]".to_string()
            }
        }
        DtoKind::Union => {
            if let Some(variants) = &dto.variants {
                let variant_exprs: Vec<String> = variants
                    .iter()
                    .map(render_variant)
                    .collect();
                variant_exprs.join("|")
            } else {
                "()".to_string()
            }
        }
        DtoKind::Paginated => {
            // Paginated DTOs render as their reference name (structural detail in canonical)
            extract_name(&dto.id)
        }
    };

    if dto.nullable {
        format!("{}?", base)
    } else {
        base
    }
}

pub fn render_field_type_expr(field: &DtoExtra) -> String {
    let base = if field.ty.starts_with("dto:") {
        extract_name(&field.ty)
    } else {
        field.ty.clone()
    };

    if field.nullable {
        format!("{}?", base)
    } else {
        base
    }
}

fn render_variant(variant: &DtoVariant) -> String {
    let base = if let Some(ref_) = &variant.ref_ {
        if ref_.starts_with("dto:") {
            extract_name(ref_)
        } else {
            ref_.clone()
        }
    } else if let Some(ty) = &variant.ty {
        ty.clone()
    } else {
        "unknown".to_string()
    };

    if variant.array {
        format!("{}[]", base)
    } else {
        base
    }
}

fn extract_name(id: &str) -> String {
    if let Some(stripped) = id.strip_prefix("dto:") {
        stripped.to_string()
    } else {
        id.to_string()
    }
}

pub fn render_expr_ast(expr: &TypeExpr) -> String {
    match expr {
        TypeExpr::Primitive(p) => p.as_str().to_string(),
        TypeExpr::Reference(name) => name.clone(),
        TypeExpr::Array(inner) => format!("{}[]", render_expr_ast(inner)),
        TypeExpr::Nullable(inner) => format!("{}?", render_expr_ast(inner)),
        TypeExpr::Union(variants) => {
            let exprs: Vec<String> = variants.iter().map(render_expr_ast).collect();
            exprs.join("|")
        }
        TypeExpr::Map { key, value } => {
            format!("Map<{}, {}>", render_expr_ast(key), render_expr_ast(value))
        }
        TypeExpr::Unit => "()".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_object() {
        let dto = DtoDefinition {
            id: "dto:LoginUser".to_string(),
            kind: DtoKind::Object,
            base: Some("entity:user".to_string()),
            state: "accepted".to_string(),
            created_by: "test".to_string(),
            values: None,
            value_type: None,
            key_type: None,
            nullable: false,
            items: None,
            variants: None,
            discriminator: None,
            cursor_field: None,
            has_more_field: None,
            total_field: None,
            meta_fields: None,
            wrapper: false,
            wraps: None,
            pick: Some(vec!["email".to_string(), "password".to_string()]),
            omit: None,
            required: vec![],
            extras: Default::default(),
            source_anchors: vec![],
            decision_refs: vec![],
            confidence: None,
        };
        assert_eq!(render_type_expr(&dto), "LoginUser");
    }

    #[test]
    fn test_render_map() {
        let dto = DtoDefinition {
            id: "dto:PrepareUploadResponse".to_string(),
            kind: DtoKind::Map,
            base: None,
            state: "accepted".to_string(),
            created_by: "test".to_string(),
            values: None,
            value_type: Some("string".to_string()),
            key_type: Some("string".to_string()),
            nullable: false,
            items: None,
            variants: None,
            discriminator: None,
            cursor_field: None,
            has_more_field: None,
            total_field: None,
            meta_fields: None,
            wrapper: false,
            wraps: None,
            pick: None,
            omit: None,
            required: vec![],
            extras: Default::default(),
            source_anchors: vec![],
            decision_refs: vec![],
            confidence: None,
        };
        assert_eq!(render_type_expr(&dto), "Map<string, string>");
    }

    #[test]
    fn test_render_array_alias() {
        let dto = DtoDefinition {
            id: "dto:BillArray".to_string(),
            kind: DtoKind::ArrayAlias,
            base: None,
            state: "accepted".to_string(),
            created_by: "test".to_string(),
            values: None,
            value_type: None,
            key_type: None,
            nullable: false,
            items: Some("dto:Bill".to_string()),
            variants: None,
            discriminator: None,
            cursor_field: None,
            has_more_field: None,
            total_field: None,
            meta_fields: None,
            wrapper: false,
            wraps: None,
            pick: None,
            omit: None,
            required: vec![],
            extras: Default::default(),
            source_anchors: vec![],
            decision_refs: vec![],
            confidence: None,
        };
        assert_eq!(render_type_expr(&dto), "Bill[]");
    }
}
