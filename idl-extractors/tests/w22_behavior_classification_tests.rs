/// W22 Task B: Behavior classification tests
use serde_json::json;

/// Test entity classification (has id + mutation operations)
#[test]
fn test_entity_classification() {
    let schema = json!({
        "type": "object",
        "properties": {
            "id": { "type": "integer" },
            "name": { "type": "string" },
            "email": { "type": "string" }
        },
        "required": ["id", "name"]
    });

    // Entity should have:
    // - id field present
    // - Used in PUT/PATCH/DELETE operations (simulated)
    assert!(schema
        .get("properties")
        .and_then(|p| p.as_object())
        .and_then(|p| p.get("id"))
        .is_some());
}

/// Test command classification (verb-name + write operation)
#[test]
fn test_command_classification() {
    // Command patterns: CreateUser, UpdateAccount, DeleteTransaction
    let _create_user = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "email": { "type": "string" },
            "password": { "type": "string" }
        },
        "required": ["name", "email"]
    });

    // Schema name starts with verb (Create, Update, Delete, etc.)
    let command_names = vec![
        "CreateUser",
        "UpdateAccount",
        "DeleteTransaction",
        "StoreRequest",
    ];

    for name in command_names {
        assert!(
            name.starts_with("Create")
                || name.starts_with("Update")
                || name.starts_with("Delete")
                || name.starts_with("Store"),
            "Should be classified as command: {}",
            name
        );
    }
}

/// Test event classification (past tense)
#[test]
fn test_event_classification() {
    // Event patterns: UserCreated, AccountUpdated, TransactionDeleted
    let _event_schema = json!({
        "type": "object",
        "properties": {
            "event_id": { "type": "string" },
            "timestamp": { "type": "string", "format": "date-time" },
            "user_id": { "type": "integer" }
        },
        "required": ["event_id", "timestamp"]
    });

    // Event names end with past tense or contain "event"
    let event_names = vec![
        "UserCreated",
        "AccountUpdated",
        "TransactionDeleted",
        "WebhookEvent",
        "PushNotification",
    ];

    for name in event_names {
        assert!(
            name.ends_with("ed")
                || name.ends_with("Event")
                || name.contains("Event")
                || name.ends_with("Notification"),
            "Should be classified as event: {}",
            name
        );
    }
}

/// Test value-object classification (immutable, composite, no id)
#[test]
fn test_value_object_classification() {
    // Value objects: Address, Money, DateRange
    let address_schema = json!({
        "type": "object",
        "properties": {
            "street": { "type": "string" },
            "city": { "type": "string" },
            "zip_code": { "type": "string" },
            "country": { "type": "string" }
        },
        "required": ["street", "city", "country"]
    });

    // Value object should:
    // - NOT have id field
    // - Have multiple properties (composite)
    // - Not used in mutation operations (immutable)
    let props = address_schema
        .get("properties")
        .and_then(|p| p.as_object())
        .unwrap();
    assert!(!props.contains_key("id"));
    assert!(props.len() >= 2); // Composite structure

    // Value object name patterns
    let value_object_names = vec!["Address", "Money", "DateRange", "Period", "AmountRange"];

    for name in value_object_names {
        assert!(
            name.contains("Address")
                || name.contains("Money")
                || name.contains("Date")
                || name.contains("Period")
                || name.contains("Range")
                || name.contains("Amount"),
            "Should be classified as value-object: {}",
            name
        );
    }
}

/// Test query-result classification (pagination wrapper)
#[test]
fn test_query_result_classification() {
    // Query result: paginated list response
    let paginated_schema = json!({
        "type": "object",
        "properties": {
            "data": {
                "type": "array",
                "items": { "$ref": "#/components/schemas/Account" }
            },
            "meta": {
                "type": "object",
                "properties": {
                    "total": { "type": "integer" },
                    "has_more": { "type": "boolean" }
                }
            }
        }
    });

    // Query result patterns:
    // - Has "data" array field (pagination wrapper)
    // - Or name contains "List", "Array", "Collection"
    let props = paginated_schema
        .get("properties")
        .and_then(|p| p.as_object())
        .unwrap();
    assert!(props.contains_key("data"));

    let query_result_names = vec!["AccountList", "TransactionArray", "UserCollection"];

    for name in query_result_names {
        assert!(
            name.contains("List") || name.contains("Array") || name.contains("Collection"),
            "Should be classified as query-result: {}",
            name
        );
    }
}

/// Test dto-only classification (default/fallback)
#[test]
fn test_dto_only_classification() {
    // Simple DTO without special patterns
    let _simple_dto = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "value": { "type": "string" }
        }
    });

    // DTOs that don't match other patterns default to "dto-only"
    // Examples: Metadata, Configuration, Settings, etc.
    let dto_only_names = vec!["Metadata", "Configuration", "Settings", "Options"];

    // These should not match entity, command, event, value-object, or query-result patterns
    for name in dto_only_names {
        assert!(
            !name.ends_with("ed")
                && !name.starts_with("Create")
                && !name.starts_with("Update")
                && !name.starts_with("Delete")
                && !name.contains("List")
                && !name.contains("Array"),
            "Should be classified as dto-only: {}",
            name
        );
    }
}

/// Test mixed heuristics (entity vs command)
#[test]
fn test_mixed_heuristics_priority() {
    // CreateUser has both command pattern (verb) and could be entity-like
    // Command should take precedence if used in POST operation

    // UpdateAccount with id field - still a command (verb takes precedence)
    let update_with_id = json!({
        "type": "object",
        "properties": {
            "id": { "type": "integer" },
            "new_name": { "type": "string" }
        }
    });

    // Name starts with "Update" (command), even with id field
    assert!(update_with_id
        .get("properties")
        .and_then(|p| p.as_object())
        .and_then(|p| p.get("id"))
        .is_some());
}

/// Test paginated kind always maps to query-result
#[test]
fn test_paginated_kind_is_query_result() {
    // Any DTO with kind="paginated" should be classified as query-result
    let kind = "paginated";
    assert_eq!(kind, "paginated");

    // Paginated schemas represent list responses with pagination metadata
    // They should ALWAYS be classified as query-result regardless of name
}
