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

/// Wave 10 Bug 1 — multiple operations on the same path must emit a single
/// path key with all methods nested underneath. The previous implementation
/// emitted one `<path>:` block per op which YAML silently de-duped on parse.
#[test]
fn openapi_emitter_groups_methods_under_one_path_key() {
    // Two ops sharing the same path `/articles`.
    let nodes = vec![
        node(
            "operation:list-articles",
            "operation",
            json!({"name":"list-articles","inputs":[],"outputs":[]}),
        ),
        node(
            "operation:create-article",
            "operation",
            json!({
                "name":"create-article",
                "inputs":[{"name":"title","type":"string"}],
                "outputs":[]
            }),
        ),
        node(
            "api:rest",
            "api",
            json!({
                "name":"rest",
                "endpoints":[
                    {"method":"GET","path":"/articles","operation_id":"operation:list-articles"},
                    {"method":"POST","path":"/articles","operation_id":"operation:create-article"}
                ]
            }),
        ),
    ];
    let g = GraphDoc { version: "0.1.0".into(), metadata: json!({"project":"x"}),
        nodes, edges: vec![], extensions: None };
    let r = OpenApiEmitter.emit(&g).unwrap();
    let y = &r.files[0].content;
    // Exactly ONE `/articles:` path key.
    let occurrences = y.matches("\n  /articles:\n").count();
    assert_eq!(occurrences, 1, "expected one /articles path key, got:\n{y}");
    // Both methods present.
    assert!(y.contains("    get:"), "missing get under /articles");
    assert!(y.contains("    post:"), "missing post under /articles");
    // YAML is parseable and round-trips both ops on the same path.
    let parsed: serde_yaml::Value = serde_yaml::from_str(y).expect("yaml parses");
    let paths = parsed.get("paths").unwrap();
    let articles = paths.get("/articles").expect("/articles path present");
    assert!(articles.get("get").is_some());
    assert!(articles.get("post").is_some());
}

/// Wave 10 Bug 2 — request bodies must be referenced via
/// `$ref: '#/components/schemas/Body<OpName>'` and the corresponding schema
/// emitted under `components.schemas`, not inlined per-operation.
#[test]
fn openapi_emitter_emits_components_schemas_with_refs() {
    let g = fixture();
    let r = OpenApiEmitter.emit(&g).unwrap();
    let y = &r.files[0].content;

    // Entity schemas appear under components.schemas.
    let parsed: serde_yaml::Value = serde_yaml::from_str(y).expect("yaml parses");
    let schemas = parsed
        .get("components")
        .and_then(|c| c.get("schemas"))
        .expect("components.schemas present");
    assert!(schemas.get("User").is_some(), "User schema missing");
    assert!(schemas.get("Article").is_some(), "Article schema missing");

    // Body stub schema for create-article exists.
    assert!(
        schemas.get("BodyCreateArticle").is_some(),
        "BodyCreateArticle stub missing in:\n{y}"
    );

    // The create-article operation references the body schema instead of
    // inlining `type: object\nproperties:` under requestBody.
    let post = parsed
        .get("paths")
        .and_then(|p| p.get("/articles"))
        .and_then(|p| p.get("post"))
        .expect("post /articles");
    let schema_ref = post
        .get("requestBody")
        .and_then(|b| b.get("content"))
        .and_then(|c| c.get("application/json"))
        .and_then(|j| j.get("schema"))
        .and_then(|s| s.get("$ref"))
        .and_then(|v| v.as_str())
        .expect("requestBody schema $ref");
    assert_eq!(schema_ref, "#/components/schemas/BodyCreateArticle");
}
