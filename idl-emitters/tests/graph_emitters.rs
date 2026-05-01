//! Snapshot-style tests for graph emitters (Wave 8 R3).

use idl_emitters::{GraphEmitter, OpenApiEmitter, RustEmitter, TypeScriptEmitter};
use idl_graph::{EdgeDoc, GraphDoc, NodeDoc};
use serde_json::json;

fn node(id: &str, kind: &str, props: serde_json::Value) -> NodeDoc {
    NodeDoc {
        id: id.into(),
        kind: kind.into(),
        state: "accepted".into(),
        created_by: Some("ai".into()),
        props: props.as_object().unwrap().clone(),
        source_anchors: vec![],
        confidence: None,
        decision_refs: vec![],
    }
}

fn fixture() -> GraphDoc {
    let nodes = vec![
        node("entity:user", "entity", json!({
            "name": "User",
            "attributes": [
                {"name":"id","type":"int","unique":true},
                {"name":"email","type":"string","unique":true},
                {"name":"bio","type":"string","nullable":true}
            ]
        })),
        node("entity:article", "entity", json!({
            "name": "Article",
            "attributes": [
                {"name":"slug","type":"string","unique":true},
                {"name":"title","type":"string"}
            ]
        })),
        node("entity:tag", "entity", json!({"name":"Tag","attributes":[{"name":"name","type":"string"}]})),
        node("operation:create-article", "operation", json!({
            "name":"create-article",
            "inputs":[{"name":"title","type":"string"},{"name":"body","type":"string"}],
            "outputs":[{"name":"article","type":"Article"}]
        })),
        node("operation:get-tags", "operation", json!({
            "name":"get-tags","inputs":[],"outputs":[{"name":"tags","type":"json"}]
        })),
        node("api:fixture", "api", json!({
            "name":"fixture",
            "protocol":"rest",
            "endpoints":[
                {"method":"POST","path":"/articles","operation_id":"operation:create-article"},
                {"method":"GET","path":"/tags","operation_id":"operation:get-tags"}
            ]
        })),
    ];
    let edges = vec![EdgeDoc {
        id: "e1".into(),
        kind: "implements".into(),
        from: "api:fixture".into(),
        to: "operation:create-article".into(),
        props: serde_json::Map::new(),
    }];
    GraphDoc {
        version: "0.1.0".into(),
        metadata: json!({"project":"fixture"}),
        nodes,
        edges,
        extensions: None,
    }
}

#[test]
fn rust_emitter_covers_kernel_kinds() {
    let g = fixture();
    let r = RustEmitter.emit(&g).unwrap();
    let entities = r.files.iter().find(|f| f.path.ends_with("entities.rs")).expect("entities.rs");
    assert!(entities.content.contains("pub struct User"));
    assert!(entities.content.contains("pub struct Article"));
    assert!(entities.content.contains("// GENERATED_FROM entity:user"));
    let ops = r.files.iter().find(|f| f.path.ends_with("operations.rs")).unwrap();
    assert!(ops.content.contains("trait Operations"));
    assert!(ops.content.contains("fn create_article"));
    let routes = r.files.iter().find(|f| f.path.ends_with("routes.rs")).unwrap();
    assert!(routes.content.contains(".route(\"/articles\", post"));
    assert!(r.total_loc() > 20);
}

#[test]
fn typescript_emitter_covers_kernel_kinds() {
    let g = fixture();
    let r = TypeScriptEmitter.emit(&g).unwrap();
    let ents = r.files.iter().find(|f| f.path.ends_with("entities.ts")).unwrap();
    assert!(ents.content.contains("export interface User"));
    assert!(ents.content.contains("bio?: string"));
    let api = r.files.iter().find(|f| f.path.ends_with("api.ts")).unwrap();
    assert!(api.content.contains("FixtureRoutes"));
    assert!(api.content.contains("\"/articles\""));
}

#[test]
fn openapi_emitter_emits_yaml() {
    let g = fixture();
    let r = OpenApiEmitter.emit(&g).unwrap();
    assert_eq!(r.files.len(), 1);
    let y = &r.files[0].content;
    assert!(y.contains("openapi: 3.1.0"));
    assert!(y.contains("/articles:"));
    assert!(y.contains("User:"));
}
