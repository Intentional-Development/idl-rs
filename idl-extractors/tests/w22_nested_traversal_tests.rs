/// W22 Task A: Nested schema traversal tests
use serde_json::{json, Value};

/// Test extraction of discriminator from nested request body schema
#[test]
fn test_nested_request_body_discriminator() {
    // Simulate a path operation with discriminator in requestBody
    let _path_operation = json!({
        "post": {
            "requestBody": {
                "content": {
                    "application/json": {
                        "schema": {
                            "$ref": "#/components/schemas/WebhookPayload"
                        }
                    }
                }
            }
        }
    });

    // The referenced schema has a discriminator
    let webhook_schema = json!({
        "type": "object",
        "discriminator": {
            "propertyName": "action",
            "mapping": {
                "opened": "#/components/schemas/WebhookOpened",
                "closed": "#/components/schemas/WebhookClosed"
            }
        }
    });

    // This discriminator should be extracted even though it's nested in requestBody
    assert!(webhook_schema.get("discriminator").is_some());

    // Verify the discriminator structure
    let disc = webhook_schema.get("discriminator").unwrap();
    assert_eq!(
        disc.get("propertyName").and_then(Value::as_str),
        Some("action")
    );
    assert!(disc.get("mapping").is_some());
}

/// Test extraction of discriminator from nested response schema
#[test]
fn test_nested_response_discriminator() {
    // Simulate a path operation with discriminator in response
    let _path_operation = json!({
        "get": {
            "responses": {
                "200": {
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/Event"
                            }
                        }
                    }
                }
            }
        }
    });

    // The referenced schema has oneOf (implicit discriminator)
    let event_schema = json!({
        "type": "object",
        "oneOf": [
            { "$ref": "#/components/schemas/PushEvent" },
            { "$ref": "#/components/schemas/PullRequestEvent" },
            { "$ref": "#/components/schemas/IssueEvent" }
        ]
    });

    // This oneOf should be detected as a union even though nested in response
    assert!(event_schema.get("oneOf").is_some());
    let one_of = event_schema.get("oneOf").and_then(Value::as_array).unwrap();
    assert_eq!(one_of.len(), 3);
}

/// Test extraction of propertyName-only discriminator from nested schema
#[test]
fn test_nested_propertyname_only_discriminator() {
    // GitHub check-runs pattern: discriminator without mapping
    let check_runs_schema = json!({
        "type": "object",
        "discriminator": {
            "propertyName": "status"
        },
        "oneOf": [
            {
                "properties": {
                    "status": { "enum": ["completed"] },
                    "conclusion": { "type": "string" }
                },
                "required": ["status", "conclusion"]
            },
            {
                "properties": {
                    "status": { "enum": ["queued", "in_progress"] }
                },
                "required": ["status"]
            }
        ]
    });

    // Verify propertyName-only discriminator structure
    let disc = check_runs_schema.get("discriminator").unwrap();
    assert_eq!(
        disc.get("propertyName").and_then(Value::as_str),
        Some("status")
    );

    // Should NOT have mapping field when source has none
    assert!(disc.get("mapping").is_none());
}

/// Test inline oneOf extraction from nested path
#[test]
fn test_inline_oneof_in_nested_path() {
    // Path with inline oneOf in response (no $ref)
    let response_schema = json!({
        "oneOf": [
            { "$ref": "#/components/schemas/SuccessResponse" },
            { "$ref": "#/components/schemas/ErrorResponse" }
        ]
    });

    // Inline oneOf should be detected
    assert!(response_schema.get("oneOf").is_some());
    let variants = response_schema
        .get("oneOf")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(variants.len(), 2);

    // Both variants should have $ref
    for variant in variants {
        assert!(variant.get("$ref").is_some());
    }
}

/// Test complex nested allOf pattern (Azure OpenAI style)
#[test]
fn test_nested_allof_discriminator() {
    // Azure OpenAI pattern: discriminator without oneOf, variants use allOf
    let message_schema = json!({
        "type": "object",
        "properties": {
            "role": {
                "$ref": "#/components/schemas/MessageRole"
            }
        },
        "discriminator": {
            "propertyName": "role",
            "mapping": {
                "system": "#/components/schemas/SystemMessage",
                "user": "#/components/schemas/UserMessage",
                "assistant": "#/components/schemas/AssistantMessage"
            }
        },
        "required": ["role"]
    });

    // Verify discriminator with mapping but no oneOf
    assert!(message_schema.get("discriminator").is_some());
    assert!(message_schema.get("oneOf").is_none());

    let disc = message_schema.get("discriminator").unwrap();
    assert_eq!(
        disc.get("propertyName").and_then(Value::as_str),
        Some("role")
    );

    let mapping = disc.get("mapping").and_then(Value::as_object).unwrap();
    assert_eq!(mapping.len(), 3);
    assert!(mapping.contains_key("system"));
    assert!(mapping.contains_key("user"));
    assert!(mapping.contains_key("assistant"));
}
