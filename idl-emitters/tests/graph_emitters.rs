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
        node(
            "entity:user",
            "entity",
            json!({
                "name": "User",
                "attributes": [
                    {"name":"id","type":"int","unique":true},
                    {"name":"email","type":"string","unique":true},
                    {"name":"bio","type":"string","nullable":true}
                ]
            }),
        ),
        node(
            "entity:article",
            "entity",
            json!({
                "name": "Article",
                "attributes": [
                    {"name":"slug","type":"string","unique":true},
                    {"name":"title","type":"string"}
                ]
            }),
        ),
        node(
            "entity:tag",
            "entity",
            json!({"name":"Tag","attributes":[{"name":"name","type":"string"}]}),
        ),
        node(
            "operation:create-article",
            "operation",
            json!({
                "name":"create-article",
                "inputs":[{"name":"title","type":"string"},{"name":"body","type":"string"}],
                "outputs":[{"name":"article","type":"Article"}]
            }),
        ),
        node(
            "operation:get-tags",
            "operation",
            json!({
                "name":"get-tags","inputs":[],"outputs":[{"name":"tags","type":"json"}]
            }),
        ),
        node(
            "api:fixture",
            "api",
            json!({
                "name":"fixture",
                "protocol":"rest",
                "endpoints":[
                    {"method":"POST","path":"/articles","operation_id":"operation:create-article"},
                    {"method":"GET","path":"/tags","operation_id":"operation:get-tags"}
                ]
            }),
        ),
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
    let entities = r
        .files
        .iter()
        .find(|f| f.path.ends_with("entities.rs"))
        .expect("entities.rs");
    assert!(entities.content.contains("pub struct User"));
    assert!(entities.content.contains("pub struct Article"));
    assert!(entities.content.contains("// GENERATED_FROM entity:user"));
    let ops = r
        .files
        .iter()
        .find(|f| f.path.ends_with("operations.rs"))
        .unwrap();
    assert!(ops.content.contains("trait Operations"));
    assert!(ops.content.contains("fn create_article"));
    let routes = r
        .files
        .iter()
        .find(|f| f.path.ends_with("routes.rs"))
        .unwrap();
    assert!(routes.content.contains(".route(\"/articles\", post"));
    assert!(r.total_loc() > 20);
}

#[test]
fn typescript_emitter_covers_kernel_kinds() {
    let g = fixture();
    let r = TypeScriptEmitter.emit(&g).unwrap();
    let ents = r
        .files
        .iter()
        .find(|f| f.path.ends_with("entities.ts"))
        .unwrap();
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
    let g = GraphDoc {
        version: "0.1.0".into(),
        metadata: json!({"project":"x"}),
        nodes,
        edges: vec![],
        extensions: None,
    };
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

/// Wave 12 — DTO referenced via `operation.props.accepts.dto` is emitted
/// under `components.schemas.<DtoName>` as a pick projection of the base
/// entity, and the operation's requestBody points at it instead of the
/// `Body<OpName>` stub.
#[test]
fn openapi_emitter_emits_dto_projection_for_accepts_dto() {
    let nodes = vec![
        node(
            "entity:user",
            "entity",
            json!({
                "name": "User",
                "attributes": [
                    {"name":"id","type":"int","unique":true},
                    {"name":"email","type":"string"},
                    {"name":"username","type":"string"},
                    {"name":"password","type":"string"},
                    {"name":"bio","type":"string","nullable":true},
                    {"name":"image","type":"string","nullable":true}
                ]
            }),
        ),
        node(
            "operation:login-user",
            "operation",
            json!({
                "name":"login-user",
                "inputs":[{"name":"email","type":"string"},{"name":"password","type":"string"}],
                "outputs":[],
                "side_effects":[],
                "accepts": {"dto": "dto:LoginUser"}
            }),
        ),
        node(
            "api:auth",
            "api",
            json!({
                "name":"auth","protocol":"rest",
                "endpoints":[{"method":"POST","path":"/users/login","operation_id":"operation:login-user"}]
            }),
        ),
    ];
    let g = GraphDoc {
        version: "0.1.2".into(),
        metadata: json!({"project":"dto"}),
        nodes,
        edges: vec![],
        extensions: Some(json!({
            "dto": {"definitions": [
                {
                    "id": "dto:LoginUser",
                    "base": "entity:user",
                    "state": "proposed",
                    "created_by": "ai",
                    "pick": ["email", "password"],
                    "required": ["email", "password"]
                }
            ]}
        })),
    };
    let r = OpenApiEmitter.emit(&g).unwrap();
    let y = &r.files[0].content;
    let parsed: serde_yaml::Value = serde_yaml::from_str(y).expect("yaml parses");
    let schemas = parsed
        .get("components")
        .and_then(|c| c.get("schemas"))
        .unwrap();

    let login = schemas.get("LoginUser").expect("LoginUser DTO emitted");
    let props = login
        .get("properties")
        .expect("LoginUser has properties")
        .as_mapping()
        .unwrap();
    let prop_names: std::collections::BTreeSet<_> =
        props.keys().filter_map(|k| k.as_str()).collect();
    assert_eq!(
        prop_names,
        ["email", "password"].iter().copied().collect(),
        "LoginUser projection should be exactly email + password, got {prop_names:?}"
    );

    // Stub Body schema must NOT be emitted when accepts.dto is set.
    assert!(
        schemas.get("BodyLogin-user").is_none() && schemas.get("BodyLoginUser").is_none(),
        "stub body should be suppressed when accepts.dto is present"
    );

    // requestBody $ref points at the DTO.
    let post = parsed
        .get("paths")
        .and_then(|p| p.get("/users/login"))
        .and_then(|p| p.get("post"))
        .unwrap();
    let r = post
        .get("requestBody")
        .and_then(|b| b.get("content"))
        .and_then(|c| c.get("application/json"))
        .and_then(|j| j.get("schema"))
        .and_then(|s| s.get("$ref"))
        .and_then(|v| v.as_str())
        .unwrap();
    assert_eq!(r, "#/components/schemas/LoginUser");
}

/// Wave 12 — DTO with extras and omit projects (base ∖ omit) ∪ extras and
/// emits `required` exactly as declared.
#[test]
fn openapi_emitter_dto_omit_extras_required() {
    let nodes = vec![node(
        "entity:user",
        "entity",
        json!({
            "name": "User",
            "attributes": [
                {"name":"id","type":"int","unique":true},
                {"name":"email","type":"string"},
                {"name":"username","type":"string"},
                {"name":"password","type":"string"},
                {"name":"bio","type":"string","nullable":true},
                {"name":"image","type":"string","nullable":true}
            ]
        }),
    )];
    let g = GraphDoc {
        version: "0.1.2".into(),
        metadata: json!({"project":"dto"}),
        nodes,
        edges: vec![],
        extensions: Some(json!({
            "dto": {"definitions": [
                {
                    "id": "dto:User",
                    "base": "entity:user",
                    "state": "proposed",
                    "created_by": "ai",
                    "omit": ["id", "password"],
                    "extras": {"token": {"type":"string"}},
                    "required": ["email", "username", "bio", "image", "token"]
                }
            ]}
        })),
    };
    let r = OpenApiEmitter.emit(&g).unwrap();
    let y = &r.files[0].content;
    let parsed: serde_yaml::Value = serde_yaml::from_str(y).expect("yaml parses");
    let user = parsed
        .get("components")
        .and_then(|c| c.get("schemas"))
        .and_then(|s| s.get("User"))
        .expect("User DTO emitted");
    let props: std::collections::BTreeSet<_> = user
        .get("properties")
        .unwrap()
        .as_mapping()
        .unwrap()
        .keys()
        .filter_map(|k| k.as_str())
        .collect();
    assert_eq!(
        props,
        ["email", "username", "bio", "image", "token"]
            .iter()
            .copied()
            .collect()
    );
    let req: std::collections::BTreeSet<_> = user
        .get("required")
        .unwrap()
        .as_sequence()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert_eq!(
        req,
        ["email", "username", "bio", "image", "token"]
            .iter()
            .copied()
            .collect()
    );
}
