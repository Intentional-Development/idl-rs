//! OpenAPI 3.1 target — api + entities → openapi.yaml.
//!
//! Wave 10 fixes:
//! * Operations are grouped by path so each path key appears once with all
//!   methods nested under it (previously duplicate keys silently lost ops in
//!   YAML round-trip).
//! * Request bodies and entities are emitted under `components.schemas` and
//!   referenced via `$ref` instead of being inlined per-operation. Inline
//!   request bodies (DTOs not yet in the graph as entities) are emitted as
//!   stub `Body<OperationName>` schemas.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use anyhow::Result;
use idl_graph::GraphDoc;

use super::{node_name, pascal_case, EmitReport, EmittedFile, GraphEmitter};

pub struct OpenApiEmitter;

impl GraphEmitter for OpenApiEmitter {
    fn target(&self) -> &str {
        "openapi"
    }

    fn emit(&self, graph: &GraphDoc) -> Result<EmitReport> {
        let mut report = EmitReport { target: "openapi".into(), ..Default::default() };

        let title = graph
            .metadata
            .get("project")
            .and_then(|v| v.as_str())
            .unwrap_or("idl-graph")
            .to_string();

        let ops: Vec<&idl_graph::NodeDoc> = graph.nodes_of_kind("operation").collect();
        let find_op = |id: &str| ops.iter().copied().find(|n| n.id == id);

        // ---------------------------------------------------------------
        // Pass 1 — build per-path op buckets and per-operation body schemas.
        // ---------------------------------------------------------------
        struct EmittedOp<'a> {
            method: String,
            api_id: String,
            op_id: String,
            op_node: Option<&'a idl_graph::NodeDoc>,
            body_schema_name: Option<String>,
        }

        // path → ordered list of (method, op meta)
        let mut paths: BTreeMap<String, Vec<EmittedOp>> = BTreeMap::new();
        // schema-name → yaml fragment (already indented under `components.schemas`)
        let mut body_schemas: BTreeMap<String, String> = BTreeMap::new();

        for api in graph.nodes_of_kind("api") {
            let api_id = api.id.clone();
            if let Some(eps) = api.props.get("endpoints").and_then(|v| v.as_array()) {
                for ep in eps {
                    let method = ep
                        .get("method")
                        .and_then(|v| v.as_str())
                        .unwrap_or("GET")
                        .to_lowercase();
                    let path = ep.get("path").and_then(|v| v.as_str()).unwrap_or("/").to_string();
                    let opid = ep
                        .get("operation_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let op_node = find_op(&opid);

                    let mut body_schema_name: Option<String> = None;
                    if let Some(op) = op_node {
                        if let Some(inputs) = op.props.get("inputs").and_then(|v| v.as_array()) {
                            if !inputs.is_empty() && method != "get" && method != "delete" {
                                let name = format!("Body{}", pascal_case(&node_name(op)));
                                let mut s = String::new();
                                let _ = writeln!(s, "    {name}:");
                                let _ = writeln!(s, "      type: object");
                                let _ = writeln!(s, "      properties:");
                                for inp in inputs {
                                    let n = inp
                                        .get("name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("arg");
                                    let t = inp
                                        .get("type")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("string");
                                    let _ = writeln!(s, "        {n}:");
                                    let _ = writeln!(s, "          type: {}", map_type_to_openapi(t));
                                }
                                body_schemas.insert(name.clone(), s);
                                body_schema_name = Some(name);
                            }
                        }
                    }

                    paths.entry(path).or_default().push(EmittedOp {
                        method,
                        api_id: api_id.clone(),
                        op_id: opid,
                        op_node,
                        body_schema_name,
                    });
                }
            }
        }

        // ---------------------------------------------------------------
        // Pass 2 — emit yaml.
        // ---------------------------------------------------------------
        let mut s = String::new();
        let _ = writeln!(s, "# AUTO-GENERATED by idl-emitters · do not edit");
        let _ = writeln!(s, "openapi: 3.1.0");
        let _ = writeln!(s, "info:");
        let _ = writeln!(s, "  title: {title}");
        let _ = writeln!(s, "  version: {}", graph.version);
        let _ = writeln!(s, "paths:");

        let mut endpoint_count = 0;
        for (path, methods) in &paths {
            let _ = writeln!(s, "  {path}:");
            for ep in methods {
                let _ = writeln!(s, "    # GENERATED_FROM {} via {}", ep.op_id, ep.api_id);
                let _ = writeln!(s, "    {}:", ep.method);
                let _ = writeln!(s, "      operationId: {}", yaml_str(&ep.op_id));
                if let Some(op) = ep.op_node {
                    let _ = writeln!(s, "      summary: {}", yaml_str(&node_name(op)));
                }
                if let Some(body) = &ep.body_schema_name {
                    let _ = writeln!(s, "      requestBody:");
                    let _ = writeln!(s, "        content:");
                    let _ = writeln!(s, "          application/json:");
                    let _ = writeln!(s, "            schema:");
                    let _ = writeln!(s, "              $ref: '#/components/schemas/{body}'");
                }
                let _ = writeln!(s, "      responses:");
                let _ = writeln!(s, "        '200':");
                let _ = writeln!(s, "          description: OK");
                endpoint_count += 1;
            }
        }

        // ---------------------------------------------------------------
        // components.schemas — entities, variants, then body-DTO stubs.
        // ---------------------------------------------------------------
        let _ = writeln!(s, "components:");
        let _ = writeln!(s, "  schemas:");
        let mut entity_count = 0;
        let mut emitted_names: std::collections::BTreeSet<String> =
            std::collections::BTreeSet::new();

        for kind in ["entity", "variant"] {
            for node in graph.nodes_of_kind(kind) {
                let name = pascal_case(&node_name(node));
                if !emitted_names.insert(name.clone()) {
                    continue;
                }
                let _ = writeln!(s, "    # GENERATED_FROM {}", node.id);
                let _ = writeln!(s, "    {name}:");
                let _ = writeln!(s, "      type: object");
                if let Some(attrs) = node.props.get("attributes").and_then(|v| v.as_array()) {
                    if !attrs.is_empty() {
                        let _ = writeln!(s, "      properties:");
                        for attr in attrs {
                            let an = attr.get("name").and_then(|v| v.as_str()).unwrap_or("f");
                            let at = attr.get("type").and_then(|v| v.as_str()).unwrap_or("string");
                            let nullable = attr
                                .get("nullable")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            let _ = writeln!(s, "        {an}:");
                            let _ = writeln!(s, "          type: {}", map_type_to_openapi(at));
                            if nullable {
                                let _ = writeln!(s, "          nullable: true");
                            }
                        }
                        let required: Vec<&str> = attrs
                            .iter()
                            .filter(|a| {
                                !a.get("nullable").and_then(|v| v.as_bool()).unwrap_or(false)
                            })
                            .filter_map(|a| a.get("name").and_then(|v| v.as_str()))
                            .collect();
                        if !required.is_empty() {
                            let _ = writeln!(s, "      required:");
                            for r in required {
                                let _ = writeln!(s, "        - {r}");
                            }
                        }
                    }
                }
                entity_count += 1;
            }
        }

        for (name, frag) in &body_schemas {
            if emitted_names.insert(name.clone()) {
                s.push_str(frag);
            }
        }

        report.nodes_emitted = entity_count + endpoint_count;
        report.files.push(EmittedFile { path: PathBuf::from("openapi.yaml"), content: s });
        Ok(report)
    }
}

fn map_type_to_openapi(t: &str) -> String {
    match t.to_lowercase().as_str() {
        "string" | "text" | "uuid" | "datetime" | "timestamp" => "string".into(),
        "int" | "integer" | "long" | "bigint" => "integer".into(),
        "float" | "double" | "number" => "number".into(),
        "bool" | "boolean" => "boolean".into(),
        "json" => "object".into(),
        _ => "string".into(),
    }
}

fn yaml_str(s: &str) -> String {
    if s.contains(':') || s.contains('#') || s.contains('-') || s.is_empty() {
        format!("{:?}", s)
    } else {
        s.to_string()
    }
}
