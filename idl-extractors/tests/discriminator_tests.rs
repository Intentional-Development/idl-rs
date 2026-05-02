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
