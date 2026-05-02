//! Round-trip integration tests — W17 TypeExpr specification examples.
//!
//! Each test follows the pattern:
//!   1. Define canonical DtoDefinition
//!   2. Render as TypeExpr string
//!   3. Parse back to AST
//!   4. Compile AST to DtoDefinition skeleton
//!   5. Assert type shape equality (kind, nullable, shape fields)
//!
//! Per W17 design: discriminator metadata is intentionally lost in TypeExpr.

use idl_graph::extensions_dto::{DtoDefinition, DtoKind, DtoVariant};
use idl_typeexpr::{expr_to_kind, parse_type_expr, render_type_expr};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

fn make_dto_base(
    id: &str,
    kind: DtoKind,
    nullable: bool,
) -> DtoDefinition {
    DtoDefinition {
        id: id.to_string(),
        kind,
        nullable,
        state: "accepted".to_string(),
        created_by: "w17-test".to_string(),
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

fn assert_shape_eq(original: &DtoDefinition, roundtrip: &DtoDefinition) {
    assert_eq!(original.kind, roundtrip.kind, "kind mismatch");
    assert_eq!(original.nullable, roundtrip.nullable, "nullable mismatch");
    
    match original.kind {
        DtoKind::Object | DtoKind::Enum | DtoKind::Paginated => {
            // Reference types — shape is just the kind
        }
        DtoKind::Map => {
            assert_eq!(original.key_type, roundtrip.key_type, "key_type mismatch");
            assert_eq!(original.value_type, roundtrip.value_type, "value_type mismatch");
        }
        DtoKind::Unit => {
            // No additional shape
        }
        DtoKind::ArrayAlias => {
            assert_eq!(original.items, roundtrip.items, "items mismatch");
        }
        DtoKind::Union => {
            let orig_variants = original.variants.as_ref().unwrap();
            let rt_variants = roundtrip.variants.as_ref().unwrap();
            assert_eq!(orig_variants.len(), rt_variants.len(), "variant count mismatch");
            for (a, b) in orig_variants.iter().zip(rt_variants.iter()) {
                assert_eq!(a, b, "variant mismatch");
            }
        }
    }
}

#[test]
fn roundtrip_01_object() {
    // Example 1: kind:object with pick (structural detail stays in canonical)
    let mut original = make_dto_base("dto:LoginUser", DtoKind::Object, false);
    original.base = Some("entity:user".to_string());
    original.pick = Some(vec!["email".to_string(), "password".to_string()]);

    let expr_str = render_type_expr(&original);
    assert_eq!(expr_str, "LoginUser");

    let expr = parse_type_expr(&expr_str).unwrap();
    let roundtrip = expr_to_kind(&expr, "LoginUser").unwrap();

    assert_shape_eq(&original, &roundtrip);
}

#[test]
fn roundtrip_02_enum() {
    // Example 2: kind:enum (values list is structural, not type-level)
    let mut original = make_dto_base("dto:DeviceType", DtoKind::Enum, false);
    original.values = Some(vec![
        "mobile".to_string(),
        "desktop".to_string(),
        "web".to_string(),
    ]);

    let expr_str = render_type_expr(&original);
    assert_eq!(expr_str, "DeviceType");

    let expr = parse_type_expr(&expr_str).unwrap();
    let roundtrip = expr_to_kind(&expr, "DeviceType").unwrap();

    // Note: values list is not preserved (it's metadata, not type shape).
    // TypeExpr represents enum as a reference name.
    assert_eq!(roundtrip.kind, DtoKind::Object); // Ref could be object or enum
}

#[test]
fn roundtrip_03_enum_nullable() {
    // Example 3: nullable enum
    // Note: The DTO id is the canonical name. When nullable=true, the "?" is appended.
    let mut original = make_dto_base("dto:DeviceType", DtoKind::Enum, true);
    original.values = Some(vec![
        "mobile".to_string(),
        "desktop".to_string(),
        "web".to_string(),
    ]);

    let expr_str = render_type_expr(&original);
    assert_eq!(expr_str, "DeviceType?");

    let expr = parse_type_expr(&expr_str).unwrap();
    let roundtrip = expr_to_kind(&expr, "DeviceType").unwrap();

    assert!(roundtrip.nullable);
}

#[test]
fn roundtrip_04_map() {
    // Example 4: Map<string, string>
    let mut original = make_dto_base("dto:PrepareUploadResponse", DtoKind::Map, false);
    original.key_type = Some("string".to_string());
    original.value_type = Some("string".to_string());

    let expr_str = render_type_expr(&original);
    assert_eq!(expr_str, "Map<string, string>");

    let expr = parse_type_expr(&expr_str).unwrap();
    let roundtrip = expr_to_kind(&expr, "PrepareUploadResponse").unwrap();

    assert_shape_eq(&original, &roundtrip);
}

#[test]
fn roundtrip_05_unit() {
    // Example 5: Unit type
    let original = make_dto_base("dto:EmptyPayload", DtoKind::Unit, false);

    let expr_str = render_type_expr(&original);
    assert_eq!(expr_str, "()");

    let expr = parse_type_expr(&expr_str).unwrap();
    let roundtrip = expr_to_kind(&expr, "EmptyPayload").unwrap();

    assert_shape_eq(&original, &roundtrip);
}

#[test]
fn roundtrip_06_array_alias() {
    // Example 6: Array alias (Bill[])
    let mut original = make_dto_base("dto:BillArray", DtoKind::ArrayAlias, false);
    original.items = Some("dto:Bill".to_string());

    let expr_str = render_type_expr(&original);
    assert_eq!(expr_str, "Bill[]");

    let expr = parse_type_expr(&expr_str).unwrap();
    let roundtrip = expr_to_kind(&expr, "BillArray").unwrap();

    assert_shape_eq(&original, &roundtrip);
}

#[test]
fn roundtrip_07_union_refs() {
    // Example 7: Union of references (TransactionSplit|TransactionDefault)
    let mut original = make_dto_base("dto:TransactionRead", DtoKind::Union, false);
    original.variants = Some(vec![
        DtoVariant {
            ty: None,
            ref_: Some("dto:TransactionSplit".to_string()),
            array: false,
        },
        DtoVariant {
            ty: None,
            ref_: Some("dto:TransactionDefault".to_string()),
            array: false,
        },
    ]);
    // Discriminator is intentionally omitted in round-trip
    // original.discriminator = Some(...);

    let expr_str = render_type_expr(&original);
    assert_eq!(expr_str, "TransactionSplit|TransactionDefault");

    let expr = parse_type_expr(&expr_str).unwrap();
    let roundtrip = expr_to_kind(&expr, "TransactionRead").unwrap();

    assert_shape_eq(&original, &roundtrip);
    // Discriminator metadata is lost (per W17 design)
    assert!(roundtrip.discriminator.is_none());
}

#[test]
fn roundtrip_08_union_mixed() {
    // Example 8: Union with primitives and array variant
    // boolean|string|object|StringArrayItem[]
    let mut original = make_dto_base("dto:PolymorphicProperty", DtoKind::Union, false);
    original.variants = Some(vec![
        DtoVariant {
            ty: Some("boolean".to_string()),
            ref_: None,
            array: false,
        },
        DtoVariant {
            ty: Some("string".to_string()),
            ref_: None,
            array: false,
        },
        DtoVariant {
            ty: Some("object".to_string()),
            ref_: None,
            array: false,
        },
        DtoVariant {
            ty: None,
            ref_: Some("dto:StringArrayItem".to_string()),
            array: true,
        },
    ]);

    let expr_str = render_type_expr(&original);
    assert_eq!(expr_str, "boolean|string|object|StringArrayItem[]");

    let expr = parse_type_expr(&expr_str).unwrap();
    let roundtrip = expr_to_kind(&expr, "PolymorphicProperty").unwrap();

    assert_shape_eq(&original, &roundtrip);
}

#[test]
fn roundtrip_09_map_nullable() {
    // Example 9: Nullable map (Map<string, Entry>?)
    let mut original = make_dto_base("dto:NullableMap", DtoKind::Map, true);
    original.key_type = Some("string".to_string());
    original.value_type = Some("dto:Entry".to_string());

    let expr_str = render_type_expr(&original);
    assert_eq!(expr_str, "Map<string, Entry>?");

    let expr = parse_type_expr(&expr_str).unwrap();
    let roundtrip = expr_to_kind(&expr, "NullableMap").unwrap();

    assert_shape_eq(&original, &roundtrip);
}

#[test]
fn roundtrip_10_field_level() {
    // Example 10: Field-level extras (string?)
    // This is NOT a DTO-level test, but a field-level TypeExpr
    use idl_graph::extensions_dto::DtoExtra;
    use idl_typeexpr::render_field_type_expr;

    let field = DtoExtra {
        ty: "string".to_string(),
        optional: false,
        format: None,
        nullable: true,
    };

    let expr_str = render_field_type_expr(&field);
    assert_eq!(expr_str, "string?");

    let expr = parse_type_expr(&expr_str).unwrap();
    // Field-level TypeExpr does not compile to a standalone DTO
    // (bare primitives are not DTOs). This test just verifies parse round-trip.
    use idl_typeexpr::{render_expr_ast, TypeExpr, PrimitiveType};
    assert_eq!(
        expr,
        TypeExpr::Nullable(Box::new(TypeExpr::Primitive(PrimitiveType::String)))
    );
    assert_eq!(render_expr_ast(&expr), "string?");
}

// Failure mode tests

#[test]
fn test_parse_error_invalid_syntax() {
    let result = parse_type_expr("string|");
    assert!(result.is_err());
}

#[test]
fn test_parse_error_unclosed_map() {
    let result = parse_type_expr("Map<string, string");
    assert!(result.is_err());
}

#[test]
fn test_parse_error_lowercase_reference() {
    // References must start with uppercase
    let result = parse_type_expr("user");
    assert!(result.is_err());
}

#[test]
fn test_compile_error_bare_primitive() {
    use idl_typeexpr::{TypeExpr, PrimitiveType};
    let expr = TypeExpr::Primitive(PrimitiveType::String);
    let result = expr_to_kind(&expr, "Invalid");
    assert!(result.is_err());
}

#[test]
fn test_complex_nested() {
    // Complex nested expression: Map<string, User[]>?
    let mut original = make_dto_base("dto:ComplexMap", DtoKind::Map, true);
    original.key_type = Some("string".to_string());
    original.value_type = Some("dto:User".to_string()); // Note: array modifier is lost in canonical map

    // Since canonical map doesn't support array value types directly,
    // this test demonstrates a limitation. In practice, array values would be
    // modeled as a separate array-alias DTO referenced here.
    // For this test, we just verify the nullable map renders correctly.
    original.value_type = Some("dto:UserArray".to_string());

    let expr_str = render_type_expr(&original);
    assert_eq!(expr_str, "Map<string, UserArray>?");
}
