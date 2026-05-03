//! TypeExpr-augmented diagnostic formatting (W20 Phase 1).
//!
//! Per Romanoff's W19 integration plan, validation errors should render
//! type information using TypeExpr syntax for human readability. This module
//! provides utilities to format `ConstraintViolation` messages with TypeExpr
//! context when applicable.
//!
//! ## Design: Call-site formatting (Option 3)
//!
//! The `ConstraintViolation` struct in `idl-graph` is unchanged. This module
//! operates at the display layer — when printing violations to the CLI, we
//! parse DTOs from the document and augment messages that reference type shapes.

use idl_graph::extensions_dto::DtoDefinition;
use idl_typeexpr::render_type_expr;

/// Augment a message with TypeExpr rendering where "dto:Name" references appear.
///
/// This is a simple heuristic: if the message contains "dto:Name" patterns,
/// we look up the DTO and append a TypeExpr rendering in brackets for clarity.
pub fn format_message_with_dtos(message: &str, dtos: &[DtoDefinition]) -> String {
    let mut result = message.to_string();

    for dto in dtos {
        if message.contains(&dto.id) {
            let typeexpr = render_type_expr(dto);
            // Replace "dto:Name" with "dto:Name [TypeExpr: ...]"
            let pattern = dto.id.clone();
            let replacement = format!("{} [TypeExpr: {}]", pattern, typeexpr);
            result = result.replace(&pattern, &replacement);
            break; // Only augment once per message
        }
    }

    result
}

/// Format a discriminator-aware union message per §5.2 of W19 plan.
///
/// When validation errors reference discriminator logic, the error should
/// include BOTH the TypeExpr (type shape) and the discriminator property name.
///
/// Example: "union AccountHolder (discriminator: role) expects User|Admin"
#[allow(dead_code)]
pub fn format_union_discriminator_message(
    union_name: &str,
    discriminator_property: &str,
    expected_variants: &[String],
    got: Option<&str>,
) -> String {
    let variants_expr = expected_variants.join("|");
    if let Some(actual) = got {
        format!(
            "union {} (discriminator: {}) expects {}, got {}",
            union_name, discriminator_property, variants_expr, actual
        )
    } else {
        format!(
            "union {} (discriminator: {}) expects variants {}",
            union_name, discriminator_property, variants_expr
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use idl_graph::extensions_dto::{DtoKind, DtoVariant};
    use std::collections::BTreeMap;

    fn make_dto(id: &str, kind: DtoKind) -> DtoDefinition {
        DtoDefinition {
            id: id.to_string(),
            kind,
            nullable: false,
            state: "accepted".to_string(),
            created_by: "test".to_string(),
            base: None,
            values: None,
            value_type: None,
            key_type: None,
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
            extras: BTreeMap::new(),
            source_anchors: vec![],
            decision_refs: vec![],
            confidence: None,
        }
    }

    #[test]
    fn test_format_message_with_object_dto() {
        let dto = make_dto("dto:LoginUser", DtoKind::Object);
        let message = "Type mismatch in field: expected dto:LoginUser";

        let formatted = format_message_with_dtos(message, &[dto]);
        assert!(formatted.contains("TypeExpr: LoginUser"));
        assert!(formatted.contains("dto:LoginUser [TypeExpr: LoginUser]"));
    }

    #[test]
    fn test_format_message_with_array_dto() {
        let mut dto = make_dto("dto:BillArray", DtoKind::ArrayAlias);
        dto.items = Some("dto:Bill".to_string());
        let message = "Invalid type dto:BillArray";

        let formatted = format_message_with_dtos(message, &[dto]);
        assert!(formatted.contains("TypeExpr: Bill[]"));
    }

    #[test]
    fn test_format_message_with_union_dto() {
        let mut dto = make_dto("dto:AccountHolder", DtoKind::Union);
        dto.variants = Some(vec![
            DtoVariant {
                ty: None,
                ref_: Some("dto:User".to_string()),
                array: false,
            },
            DtoVariant {
                ty: None,
                ref_: Some("dto:Admin".to_string()),
                array: false,
            },
        ]);
        let message = "Union dto:AccountHolder mismatch";

        let formatted = format_message_with_dtos(message, &[dto]);
        assert!(formatted.contains("TypeExpr: User|Admin"));
    }

    #[test]
    fn test_format_message_without_dto_reference() {
        let dto = make_dto("dto:LoginUser", DtoKind::Object);
        let message = "Generic error message";

        let formatted = format_message_with_dtos(message, &[dto]);
        assert_eq!(formatted, "Generic error message");
    }

    #[test]
    fn test_format_union_discriminator_message() {
        let msg = format_union_discriminator_message(
            "AccountHolder",
            "role",
            &["User".to_string(), "Admin".to_string()],
            Some("Bot"),
        );
        assert_eq!(
            msg,
            "union AccountHolder (discriminator: role) expects User|Admin, got Bot"
        );
    }

    #[test]
    fn test_format_union_discriminator_message_no_actual() {
        let msg = format_union_discriminator_message(
            "AccountHolder",
            "role",
            &["User".to_string(), "Admin".to_string()],
            None,
        );
        assert_eq!(
            msg,
            "union AccountHolder (discriminator: role) expects variants User|Admin"
        );
    }
}
