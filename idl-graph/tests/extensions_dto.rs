//! Wave 12 — DTO extension namespace tests (RFC dto-node-kind, Direction C).

use idl_graph::{
    parse_dtos, project_field_set, validate_dtos, DtoDefinition, DtoExtra, GraphDoc, NodeDoc,
};
use serde_json::json;
use std::collections::BTreeMap;

fn entity(id: &str, name: &str, attrs: serde_json::Value) -> NodeDoc {
    NodeDoc {
        id: id.into(),
        kind: "entity".into(),
        state: "accepted".into(),
        created_by: Some("brownfield-extractor".into()),
        props: json!({
            "name": name,
            "attributes": attrs,
            "behavior_classification": "declared"
        })
        .as_object()
        .unwrap()
        .clone(),
        source_anchors: vec![],
        confidence: None,
        decision_refs: vec![],
    }
}

fn op_with_dto(id: &str, axis: &str, dto_id: &str) -> NodeDoc {
    let mut props = serde_json::Map::new();
    props.insert("name".into(), json!(id));
    props.insert("inputs".into(), json!([]));
    props.insert("outputs".into(), json!([]));
    props.insert("side_effects".into(), json!([]));
    props.insert(axis.into(), json!({"dto": dto_id}));
    NodeDoc {
        id: id.into(),
        kind: "operation".into(),
        state: "proposed".into(),
        created_by: Some("ai".into()),
        props,
        source_anchors: vec![],
        confidence: Some(idl_graph::ConfidenceDoc {
            score: 0.9,
            model: None,
            run_id: None,
        }),
        decision_refs: vec![],
    }
}

fn graph_with(dto_defs: serde_json::Value, ops: Vec<NodeDoc>) -> GraphDoc {
    let mut nodes = vec![entity(
        "entity:user",
        "User",
        json!([
            {"name":"id","type":"int","unique":true},
            {"name":"email","type":"string","unique":true},
            {"name":"username","type":"string"},
            {"name":"password","type":"string"},
            {"name":"bio","type":"string","nullable":true},
            {"name":"image","type":"string","nullable":true}
        ]),
    )];
    nodes.extend(ops);
    GraphDoc {
        version: "0.1.2".into(),
        metadata: json!({"project": "dto-test"}),
        nodes,
        edges: vec![],
        extensions: Some(json!({"dto": {"definitions": dto_defs}})),
    }
}

#[test]
fn parse_dtos_round_trip() {
    let g = graph_with(
        json!([{
            "id": "dto:LoginUser",
            "base": "entity:user",
            "state": "proposed",
            "created_by": "ai",
            "pick": ["email", "password"],
            "required": ["email", "password"]
        }]),
        vec![],
    );
    let dtos = parse_dtos(&g).expect("parse ok");
    assert_eq!(dtos.len(), 1);
    assert_eq!(dtos[0].id, "dto:LoginUser");
    assert_eq!(dtos[0].pick.as_ref().unwrap(), &vec!["email", "password"]);
}

#[test]
fn validate_happy_pick_and_required() {
    let g = graph_with(
        json!([{
            "id": "dto:LoginUser",
            "base": "entity:user",
            "state": "proposed",
            "created_by": "ai",
            "pick": ["email", "password"],
            "required": ["email", "password"]
        }]),
        vec![op_with_dto("operation:login-user", "accepts", "dto:LoginUser")],
    );
    let v = validate_dtos(&g);
    assert!(v.is_empty(), "expected no violations, got {v:?}");
}

#[test]
fn validate_happy_omit_with_extras() {
    let g = graph_with(
        json!([{
            "id": "dto:User",
            "base": "entity:user",
            "state": "proposed",
            "created_by": "ai",
            "omit": ["id", "password"],
            "extras": {"token": {"type": "string"}},
            "required": ["email", "username", "token"]
        }]),
        vec![],
    );
    let v = validate_dtos(&g);
    assert!(v.is_empty(), "expected clean validation, got {v:?}");
}

#[test]
fn validate_full_base_when_no_pick_or_omit() {
    // No pick + no omit = full projection of base entity. Still valid.
    let g = graph_with(
        json!([{
            "id": "dto:UserFull",
            "base": "entity:user",
            "state": "proposed",
            "created_by": "ai"
        }]),
        vec![],
    );
    let v = validate_dtos(&g);
    assert!(v.is_empty(), "{v:?}");
}

#[test]
fn validate_error_pick_omit_mutually_exclusive() {
    let g = graph_with(
        json!([{
            "id": "dto:Bad",
            "base": "entity:user",
            "state": "proposed",
            "created_by": "ai",
            "pick": ["email"],
            "omit": ["password"]
        }]),
        vec![],
    );
    let v = validate_dtos(&g);
    assert!(
        v.iter().any(|e| e.rule == "dto-pick-omit-exclusive"),
        "expected pick/omit exclusive error, got {v:?}"
    );
}

#[test]
fn validate_error_pick_not_subset() {
    let g = graph_with(
        json!([{
            "id": "dto:Phantom",
            "base": "entity:user",
            "state": "proposed",
            "created_by": "ai",
            "pick": ["email", "doesnotexist"]
        }]),
        vec![],
    );
    let v = validate_dtos(&g);
    assert!(v.iter().any(|e| e.rule == "dto-pick-subset"), "{v:?}");
}

#[test]
fn validate_error_base_unresolved() {
    let g = graph_with(
        json!([{
            "id": "dto:Orphan",
            "base": "entity:nope",
            "state": "proposed",
            "created_by": "ai",
            "pick": ["email"]
        }]),
        vec![],
    );
    let v = validate_dtos(&g);
    assert!(v.iter().any(|e| e.rule == "dto-base-resolves"), "{v:?}");
}

#[test]
fn validate_error_required_outside_projection() {
    let g = graph_with(
        json!([{
            "id": "dto:R",
            "base": "entity:user",
            "state": "proposed",
            "created_by": "ai",
            "pick": ["email"],
            "required": ["email", "password"]
        }]),
        vec![],
    );
    let v = validate_dtos(&g);
    assert!(
        v.iter().any(|e| e.rule == "dto-required-projected"),
        "{v:?}"
    );
}

#[test]
fn validate_error_op_dto_ref_unresolved() {
    let g = graph_with(
        json!([]),
        vec![op_with_dto(
            "operation:login-user",
            "accepts",
            "dto:Missing",
        )],
    );
    let v = validate_dtos(&g);
    assert!(
        v.iter().any(|e| e.rule == "dto-accepts-resolves"),
        "{v:?}"
    );
}

#[test]
fn validate_accepted_state_requires_anchors() {
    let g = graph_with(
        json!([{
            "id": "dto:Accepted",
            "base": "entity:user",
            "state": "accepted",
            "created_by": "brownfield-extractor",
            "pick": ["email", "password"]
        }]),
        vec![],
    );
    let v = validate_dtos(&g);
    assert!(
        v.iter().any(|e| e.rule == "dto-accepted-provenance"),
        "{v:?}"
    );
}

#[test]
fn project_field_set_omit_plus_extras() {
    use std::collections::BTreeSet;
    let dto = DtoDefinition {
        id: "dto:X".into(),
        base: "entity:user".into(),
        state: "proposed".into(),
        created_by: "ai".into(),
        pick: None,
        omit: Some(vec!["id".into(), "password".into()]),
        required: vec![],
        extras: {
            let mut m = BTreeMap::new();
            m.insert(
                "token".into(),
                DtoExtra { ty: "string".into(), optional: false, format: None },
            );
            m
        },
        source_anchors: vec![],
        decision_refs: vec![],
        confidence: None,
    };
    let base: BTreeSet<String> = ["id", "email", "username", "password", "bio", "image"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let projected = project_field_set(&dto, &base);
    let want: BTreeSet<String> =
        ["email", "username", "bio", "image", "token"].iter().map(|s| s.to_string()).collect();
    assert_eq!(projected, want);
}
