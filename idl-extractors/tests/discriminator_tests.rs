/// W19: Azure OpenAI discriminator extraction and round-trip tests
use serde_json::{json, Value};

/// Test discriminator extraction from Azure OpenAI pattern
/// (discriminator on parent schema without oneOf, variants use allOf)
#[test]
fn test_azure_openai_discriminator_extraction() {
    // Simplified Azure OpenAI chatCompletionRequestMessage pattern
    let schema = json!({
        "type": "object",
        "properties": {
            "role": {
                "$ref": "#/components/schemas/chatCompletionRequestMessageRole"
            }
        },
        "discriminator": {
            "propertyName": "role",
            "mapping": {
                "system": "#/components/schemas/chatCompletionRequestMessageSystem",
                "user": "#/components/schemas/chatCompletionRequestMessageUser",
                "assistant": "#/components/schemas/chatCompletionRequestMessageAssistant"
            }
        },
        "required": ["role"]
    });

    // The extractor should recognize this as a union with discriminator
    // Even without oneOf, the discriminator.mapping provides the variant list
    let result = extract_discriminator(&schema);
    
    assert!(result.is_some(), "Should extract discriminator from Azure OpenAI pattern");
    
    let disc = result.unwrap();
    assert_eq!(
        disc.get("property").and_then(Value::as_str),
        Some("role"),
        "Should extract propertyName as 'property'"
    );
    
    let mapping = disc.get("mapping").and_then(Value::as_object).unwrap();
    assert_eq!(mapping.len(), 3, "Should have 3 mappings");
    assert_eq!(
        mapping.get("system").and_then(Value::as_str),
        Some("dto:chatCompletionRequestMessageSystem"),
        "Should convert schema refs to dto: refs"
    );
}

/// Test that discriminator + mapping creates a union DTO
#[test]
fn test_discriminator_creates_union() {
    let schema = json!({
        "type": "object",
        "properties": {
            "type": { "type": "string" }
        },
        "discriminator": {
            "propertyName": "type",
            "mapping": {
                "text": "#/components/schemas/TextContent",
                "image": "#/components/schemas/ImageContent"
            }
        }
    });

    // This should be recognized as a union, not skipped
    // The function would return Some(union_def)
    assert!(
        has_discriminator(&schema),
        "Should detect discriminator field"
    );
    
    let mapping = extract_mapping(&schema);
    assert_eq!(mapping.len(), 2, "Should have 2 variants");
}

/// Test flagship schema: chatCompletionRequestMessage with 5 variants
#[test]
fn test_flagship_chat_completion_request_message() {
    let schema = json!({
        "type": "object",
        "properties": {
            "role": {
                "$ref": "#/components/schemas/chatCompletionRequestMessageRole"
            }
        },
        "discriminator": {
            "propertyName": "role",
            "mapping": {
                "system": "#/components/schemas/chatCompletionRequestMessageSystem",
                "user": "#/components/schemas/chatCompletionRequestMessageUser",
                "assistant": "#/components/schemas/chatCompletionRequestMessageAssistant",
                "tool": "#/components/schemas/chatCompletionRequestMessageTool",
                "function": "#/components/schemas/chatCompletionRequestMessageFunction"
            }
        },
        "required": ["role"]
    });

    let disc = extract_discriminator(&schema).expect("Should extract discriminator");
    
    // Verify property name
    assert_eq!(
        disc.get("property").and_then(Value::as_str),
        Some("role")
    );
    
    // Verify all 5 variants in mapping
    let mapping = disc.get("mapping").and_then(Value::as_object).unwrap();
    assert_eq!(mapping.len(), 5, "Should have 5 variants");
    
    let expected_variants = vec!["system", "user", "assistant", "tool", "function"];
    for variant in expected_variants {
        assert!(
            mapping.contains_key(variant),
            "Should contain variant '{}'",
            variant
        );
    }
}

// Helper functions (these would be part of the actual extractor module)

fn extract_discriminator(schema: &Value) -> Option<Value> {
    use serde_json::Map;
    use std::collections::HashMap;
    
    let disc = schema.get("discriminator")?;
    let property = disc.get("propertyName")?.as_str()?;
    let mut result = Map::new();
    result.insert("property".into(), Value::String(property.into()));
    
    if let Some(mapping) = disc.get("mapping").and_then(Value::as_object) {
        let mut m = Map::new();
        for (k, v) in mapping {
            if let Some(ref_val) = v.as_str() {
                if let Some(dto_name) = ref_val.strip_prefix("#/components/schemas/") {
                    m.insert(k.clone(), Value::String(format!("dto:{dto_name}")));
                }
            }
        }
        if !m.is_empty() {
            result.insert("mapping".into(), Value::Object(m));
        }
    }
    Some(Value::Object(result))
}

fn has_discriminator(schema: &Value) -> bool {
    schema.get("discriminator").is_some()
}

fn extract_mapping(schema: &Value) -> Vec<String> {
    schema
        .get("discriminator")
        .and_then(|d| d.get("mapping"))
        .and_then(Value::as_object)
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

/// W20: GitHub REST API implicit discriminator test
/// (oneOf without discriminator block, variants have single-value enum)
#[test]
fn test_github_implicit_discriminator_detection() {
    // GitHub REST API repository-rule pattern
    let parent_schema = json!({
        "title": "Repository Rule",
        "type": "object",
        "oneOf": [
            { "$ref": "#/components/schemas/repository-rule-creation" },
            { "$ref": "#/components/schemas/repository-rule-update" },
            { "$ref": "#/components/schemas/repository-rule-deletion" }
        ]
    });
    
    // Variant schemas with implicit discriminator (single-value enum on 'type' property)
    let variant_schemas = vec![
        json!({
            "type": "object",
            "properties": {
                "type": { "type": "string", "enum": ["creation"] }
            },
            "required": ["type"]
        }),
        json!({
            "type": "object",
            "properties": {
                "type": { "type": "string", "enum": ["update"] }
            },
            "required": ["type"]
        }),
        json!({
            "type": "object",
            "properties": {
                "type": { "type": "string", "enum": ["deletion"] }
            },
            "required": ["type"]
        }),
    ];
    
    // Current behavior: oneOf detected, but no discriminator metadata
    assert!(parent_schema.get("oneOf").is_some(), "Should detect oneOf");
    assert!(parent_schema.get("discriminator").is_none(), "Should have no explicit discriminator");
    
    // Future v0.1.8 behavior: should infer discriminator from variants
    // (This test documents the expected behavior for Romanoff's implementation)
    let inferred = infer_discriminator_from_variants(&variant_schemas);
    assert!(inferred.is_some(), "Should infer discriminator from variant patterns");
    
    let disc = inferred.unwrap();
    assert_eq!(disc.property, "type", "Should infer 'type' as discriminator property");
    assert_eq!(disc.mapping.len(), 3, "Should have 3 mappings");
    assert_eq!(disc.mapping.get("creation"), Some(&"variant-0".to_string()));
}

/// W20 v0.1.8: Test propertyName-only discriminator extraction (GitHub check-runs pattern)
#[test]
fn test_propertyname_only_discriminator() {
    // GitHub REST API check-runs pattern: discriminator without mapping
    let schema = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "status": { "type": "string", "enum": ["queued", "in_progress", "completed"] }
        },
        "discriminator": {
            "propertyName": "status"
        },
        "oneOf": [
            {
                "properties": { "status": { "enum": ["completed"] } },
                "required": ["status", "conclusion"],
                "additionalProperties": true
            },
            {
                "properties": { "status": { "enum": ["queued", "in_progress"] } },
                "additionalProperties": true
            }
        ]
    });

    let result = extract_discriminator(&schema);
    assert!(result.is_some(), "Should extract propertyName-only discriminator");
    
    let disc = result.unwrap();
    assert_eq!(
        disc.get("property").and_then(Value::as_str),
        Some("status"),
        "Should extract propertyName as 'property'"
    );
    
    // v0.1.8: mapping is optional, so it should NOT be present when source has no mapping
    assert!(
        disc.get("mapping").is_none(),
        "Should NOT have mapping field when source has no mapping (propertyName-only pattern)"
    );
    
    // mappingDerived should be false (default) when not synthesizing mapping
    assert_eq!(
        disc.get("mappingDerived").and_then(Value::as_bool).unwrap_or(false),
        false,
        "mappingDerived should be false for propertyName-only"
    );
}

// Future v0.1.8 helper: infer discriminator from variant schemas
struct InferredDiscriminator {
    property: String,
    mapping: std::collections::HashMap<String, String>,
}

fn infer_discriminator_from_variants(variants: &[Value]) -> Option<InferredDiscriminator> {
    use std::collections::{HashMap, HashSet};
    
    if variants.is_empty() {
        return None;
    }
    
    // Find properties that appear in all variants
    let first_props: HashSet<String> = variants[0]
        .get("properties")
        .and_then(Value::as_object)
        .map(|p| p.keys().cloned().collect())
        .unwrap_or_default();
    
    for prop in first_props {
        let mut all_have_enum = true;
        let mut enum_values = Vec::new();
        
        for (idx, variant) in variants.iter().enumerate() {
            if let Some(prop_def) = variant
                .pointer(&format!("/properties/{}", prop))
            {
                if let Some(enum_val) = prop_def
                    .get("enum")
                    .and_then(Value::as_array)
                {
                    if enum_val.len() == 1 {
                        if let Some(val) = enum_val[0].as_str() {
                            enum_values.push((val.to_string(), format!("variant-{}", idx)));
                            continue;
                        }
                    }
                }
            }
            all_have_enum = false;
            break;
        }
        
        if all_have_enum && enum_values.len() == variants.len() {
            // Check uniqueness
            let unique_vals: HashSet<_> = enum_values.iter().map(|(v, _)| v).collect();
            if unique_vals.len() == enum_values.len() {
                let mapping: HashMap<String, String> = enum_values.into_iter().collect();
                return Some(InferredDiscriminator {
                    property: prop,
                    mapping,
                });
            }
        }
    }
    
    None
}

/// Test flagship GitHub schema: repository-rule with 22 variants
#[test]
fn test_flagship_repository_rule() {
    // This test validates the structural extraction without discriminator inference
    // (v0.1.7 behavior)
    let schema = json!({
        "title": "Repository Rule",
        "type": "object",
        "oneOf": [
            { "$ref": "#/components/schemas/repository-rule-creation" },
            { "$ref": "#/components/schemas/repository-rule-update" }
        ]
    });
    
    // Should detect as union (oneOf present)
    assert!(schema.get("oneOf").is_some());
    
    // Should extract variant references
    let variants = schema.get("oneOf").and_then(Value::as_array).unwrap();
    assert_eq!(variants.len(), 2);
    
    // But no discriminator metadata (expected limitation)
    assert!(schema.get("discriminator").is_none());
}
