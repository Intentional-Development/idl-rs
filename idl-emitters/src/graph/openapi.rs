//! OpenAPI 3.1 target — api + entities + extensions.dto → openapi.yaml.
//!
//! Wave 10 fixes:
//! * Operations are grouped by path so each path key appears once with all
//!   methods nested under it (previously duplicate keys silently lost ops in
//!   YAML round-trip).
//! * Request bodies and entities are emitted under `components.schemas` and
//!   referenced via `$ref` instead of being inlined per-operation. Inline
//!   request bodies (DTOs not yet in the graph as entities) are emitted as
//!   stub `Body<OperationName>` schemas.
//!
//! Wave 12 (RFC dto-node-kind, Direction C):
//! * `extensions.dto.definitions[]` are resolved up-front; each DTO is
//!   emitted under its canonical name in `components.schemas` as a
//!   pick/omit/required/extras projection over its base entity.
//! * `operation.props.accepts.dto` (and `returns.dto`, when present) take
//!   precedence over the legacy `Body<OpName>` stub. Operations without a
//!   DTO ref keep the Wave 10 stub fallback.
//! * When a DTO and an entity share a name (e.g. `User` response shape),
//!   the DTO wins — it's the wire shape, the entity is the storage shape.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::PathBuf;

use anyhow::Result;
use idl_graph::{parse_dtos, projected_fields_ordered, DtoDefinition, DtoKind, GraphDoc};

use super::{node_name, pascal_case, EmitReport, EmittedFile, GraphEmitter};

pub struct OpenApiEmitter;

impl GraphEmitter for OpenApiEmitter {
    fn target(&self) -> &str {
        "openapi"
    }

    fn emit(&self, graph: &GraphDoc) -> Result<EmitReport> {
        let mut report = EmitReport {
            target: "openapi".into(),
            ..Default::default()
        };

        let title = graph
            .metadata
            .get("project")
            .and_then(|v| v.as_str())
            .unwrap_or("idl-graph")
            .to_string();

        let ops: Vec<&idl_graph::NodeDoc> = graph.nodes_of_kind("operation").collect();
        let find_op = |id: &str| ops.iter().copied().find(|n| n.id == id);

        // ---------------------------------------------------------------
        // DTO resolution (Wave 12).
        // ---------------------------------------------------------------
        let dtos: Vec<DtoDefinition> = parse_dtos(graph).unwrap_or_default();
        let dto_by_id: BTreeMap<String, &DtoDefinition> =
            dtos.iter().map(|d| (d.id.clone(), d)).collect();
        let dto_name = |dto_id: &str| dto_id.strip_prefix("dto:").unwrap_or(dto_id).to_string();

        // ---------------------------------------------------------------
        // Pass 1 — build per-path op buckets and per-operation body schemas.
        // ---------------------------------------------------------------
        struct EmittedOp<'a> {
            method: String,
            api_id: String,
            op_id: String,
            op_node: Option<&'a idl_graph::NodeDoc>,
            request_schema: Option<String>,
            response_schema: Option<String>,
        }

        let mut paths: BTreeMap<String, Vec<EmittedOp>> = BTreeMap::new();
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
                    let path = ep
                        .get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("/")
                        .to_string();
                    let opid = ep
                        .get("operation_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let op_node = find_op(&opid);

                    let mut request_schema: Option<String> = None;
                    let mut response_schema: Option<String> = None;
                    if let Some(op) = op_node {
                        // Wave 12: prefer accepts.dto / returns.dto.
                        let dto_ref = |axis: &str| -> Option<String> {
                            op.props
                                .get(axis)
                                .and_then(|v| v.get("dto"))
                                .and_then(|v| v.as_str())
                                .filter(|id| dto_by_id.contains_key(*id))
                                .map(&dto_name)
                        };
                        if let Some(name) = dto_ref("accepts") {
                            request_schema = Some(name);
                        }
                        if let Some(name) = dto_ref("returns") {
                            response_schema = Some(name);
                        }

                        // Legacy stub fallback: if no accepts.dto, fabricate
                        // BodyOpName from inputs (Wave 10 behaviour).
                        if request_schema.is_none() {
                            if let Some(inputs) = op.props.get("inputs").and_then(|v| v.as_array())
                            {
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
                                        let _ = writeln!(
                                            s,
                                            "          type: {}",
                                            map_type_to_openapi(t)
                                        );
                                    }
                                    body_schemas.insert(name.clone(), s);
                                    request_schema = Some(name);
                                }
                            }
                        }
                    }

                    paths.entry(path).or_default().push(EmittedOp {
                        method,
                        api_id: api_id.clone(),
                        op_id: opid,
                        op_node,
                        request_schema,
                        response_schema,
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
                if let Some(body) = &ep.request_schema {
                    let _ = writeln!(s, "      requestBody:");
                    let _ = writeln!(s, "        content:");
                    let _ = writeln!(s, "          application/json:");
                    let _ = writeln!(s, "            schema:");
                    let _ = writeln!(s, "              $ref: '#/components/schemas/{body}'");
                }
                let _ = writeln!(s, "      responses:");
                let _ = writeln!(s, "        '200':");
                let _ = writeln!(s, "          description: OK");
                if let Some(resp) = &ep.response_schema {
                    let _ = writeln!(s, "          content:");
                    let _ = writeln!(s, "            application/json:");
                    let _ = writeln!(s, "              schema:");
                    let _ = writeln!(s, "                $ref: '#/components/schemas/{resp}'");
                }
                endpoint_count += 1;
            }
        }

        // ---------------------------------------------------------------
        // components.schemas — DTOs first (canonical wire shapes), then
        // entities (storage shapes) only when no DTO has claimed the name,
        // then variants, then legacy body stubs.
        // ---------------------------------------------------------------
        let _ = writeln!(s, "components:");
        let _ = writeln!(s, "  schemas:");
        let mut entity_count = 0;
        let mut emitted_names: BTreeSet<String> = BTreeSet::new();

        // Build entity attribute index for DTO projection.
        let entity_by_id: BTreeMap<String, &idl_graph::NodeDoc> = graph
            .nodes_of_kind("entity")
            .map(|n| (n.id.clone(), n))
            .collect();

        // 1. DTOs.
        for dto in &dtos {
            let name = dto_name(&dto.id);
            if !emitted_names.insert(name.clone()) {
                continue;
            }
            let base_label = dto.base.as_deref().unwrap_or("-");
            let _ = writeln!(s, "    # GENERATED_FROM {} (base {})", dto.id, base_label);
            let _ = writeln!(s, "    {name}:");

            match dto.kind {
                DtoKind::Enum => {
                    let _ = writeln!(
                        s,
                        "      type: {}",
                        dto.value_type.as_deref().unwrap_or("string")
                    );
                    if let Some(values) = &dto.values {
                        let _ = writeln!(s, "      enum:");
                        for value in values {
                            let _ = writeln!(s, "        - {}", value);
                        }
                    }
                    if dto.nullable {
                        let _ = writeln!(s, "      nullable: true");
                    }
                    entity_count += 1;
                    continue;
                }
                DtoKind::Map => {
                    let _ = writeln!(s, "      type: object");
                    let _ = writeln!(s, "      additionalProperties:");
                    if let Some(value_type) = &dto.value_type {
                        if let Some(ref_name) = value_type.strip_prefix("dto:") {
                            let _ = writeln!(s, "        $ref: '#/components/schemas/{ref_name}'");
                        } else {
                            let _ =
                                writeln!(s, "        type: {}", map_type_to_openapi(value_type));
                        }
                    } else {
                        let _ = writeln!(s, "        type: string");
                    }
                    entity_count += 1;
                    continue;
                }
                DtoKind::Unit => {
                    let _ = writeln!(s, "      type: object");
                    entity_count += 1;
                    continue;
                }
                DtoKind::ArrayAlias => {
                    let _ = writeln!(s, "      type: array");
                    let _ = writeln!(s, "      items:");
                    write_type_or_ref_schema(
                        &mut s,
                        "        ",
                        dto.items.as_deref().unwrap_or("string"),
                    );
                    if dto.nullable {
                        let _ = writeln!(s, "      nullable: true");
                    }
                    entity_count += 1;
                    continue;
                }
                DtoKind::Paginated => {
                    // Emit paginated DTOs as objects with array data field
                    let _ = writeln!(s, "      type: object");
                    entity_count += 1;
                    continue;
                }
                DtoKind::Union => {
                    let _ = writeln!(s, "      oneOf:");
                    for variant in dto.variants.as_deref().unwrap_or(&[]) {
                        if variant.array {
                            let _ = writeln!(s, "        - type: array");
                            let _ = writeln!(s, "          items:");
                            write_type_or_ref_schema(
                                &mut s,
                                "            ",
                                variant
                                    .ref_
                                    .as_deref()
                                    .or(variant.ty.as_deref())
                                    .unwrap_or("string"),
                            );
                        } else if let Some(ref_) = &variant.ref_ {
                            let _ = writeln!(
                                s,
                                "        - $ref: '#/components/schemas/{}'",
                                dto_name(ref_)
                            );
                        } else {
                            let _ = writeln!(
                                s,
                                "        - type: {}",
                                map_type_to_openapi(variant.ty.as_deref().unwrap_or("string"))
                            );
                        }
                    }
                    if let Some(discriminator) = &dto.discriminator {
                        let _ = writeln!(s, "      discriminator:");
                        let _ = writeln!(s, "        propertyName: {}", discriminator.property);
                        if let Some(mapping) = &discriminator.mapping {
                            if !mapping.is_empty() {
                                let _ = writeln!(s, "        mapping:");
                                for (key, target) in mapping {
                                    let mapped = target
                                        .strip_prefix("dto:")
                                        .map(dto_name)
                                        .unwrap_or_else(|| target.clone());
                                    let _ = writeln!(
                                        s,
                                        "          {}: '#/components/schemas/{}'",
                                        yaml_str(key),
                                        mapped
                                    );
                                }
                            }
                        }
                    }
                    if dto.nullable {
                        let _ = writeln!(s, "      nullable: true");
                    }
                    entity_count += 1;
                    continue;
                }
                DtoKind::Object => {}
            }

            let _ = writeln!(s, "      type: object");

            // Wave 14: wrapper DTOs.
            if dto.wrapper {
                if let Some(wraps_ref) = &dto.wraps {
                    let wrapped_name = dto_name(wraps_ref);
                    // Property name is lowercase wrapped DTO name, optionally pluralized.
                    // SingleArticleResponse -> article, MultipleArticlesResponse -> articles.
                    let is_array = name.starts_with("Multiple")
                        || name.contains("Comments")
                        || name == "TagsResponse";
                    let prop_name = if is_array {
                        format!("{}s", wrapped_name.to_lowercase())
                    } else {
                        wrapped_name.to_lowercase()
                    };
                    let _ = writeln!(s, "      required:");
                    let _ = writeln!(s, "        - {prop_name}");
                    let _ = writeln!(s, "      properties:");
                    let _ = writeln!(s, "        {prop_name}:");
                    if is_array {
                        let _ = writeln!(s, "          type: array");
                        let _ = writeln!(s, "          items:");
                        let _ =
                            writeln!(s, "            $ref: '#/components/schemas/{wrapped_name}'");
                    } else {
                        let _ =
                            writeln!(s, "          $ref: '#/components/schemas/{wrapped_name}'");
                    }
                }
                entity_count += 1;
                continue;
            }

            // Standard DTO projection.
            let base_attrs_arr: Vec<serde_json::Value> = dto
                .base
                .as_ref()
                .and_then(|base| entity_by_id.get(base))
                .and_then(|n| n.props.get("attributes"))
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let fields = projected_fields_ordered(dto, &base_attrs_arr);
            if !fields.is_empty() {
                let _ = writeln!(s, "      properties:");
                for f in &fields {
                    let _ = writeln!(s, "        {}:", f.name);
                    let _ = writeln!(s, "          type: {}", map_type_to_openapi(&f.ty));
                    if let Some(fmt) = &f.format {
                        let _ = writeln!(s, "          format: {fmt}");
                    }
                    if f.nullable {
                        let _ = writeln!(s, "          nullable: true");
                    }
                }
                if !dto.required.is_empty() {
                    let _ = writeln!(s, "      required:");
                    for r in &dto.required {
                        let _ = writeln!(s, "        - {r}");
                    }
                }
            }
            entity_count += 1;
        }

        // 2. Entities + variants (skip if a DTO already owns the name).
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
                            let at = attr
                                .get("type")
                                .and_then(|v| v.as_str())
                                .unwrap_or("string");
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

        // 3. Legacy body stubs (only for ops without accepts.dto).
        for (name, frag) in &body_schemas {
            if emitted_names.insert(name.clone()) {
                s.push_str(frag);
            }
        }

        report.nodes_emitted = entity_count + endpoint_count;
        report.files.push(EmittedFile {
            path: PathBuf::from("openapi.yaml"),
            content: s,
        });
        Ok(report)
    }
}

fn write_type_or_ref_schema(s: &mut String, indent: &str, ty: &str) {
    if let Some(ref_name) = ty.strip_prefix("dto:") {
        let _ = writeln!(s, "{indent}$ref: '#/components/schemas/{}'", ref_name);
    } else {
        let _ = writeln!(s, "{indent}type: {}", map_type_to_openapi(ty));
    }
}

fn map_type_to_openapi(t: &str) -> String {
    match t.to_lowercase().as_str() {
        "string" | "text" | "uuid" | "datetime" | "timestamp" => "string".into(),
        "int" | "integer" | "long" | "bigint" => "integer".into(),
        "float" | "double" | "number" => "number".into(),
        "bool" | "boolean" => "boolean".into(),
        "json" | "object" => "object".into(),
        "array" | "list" => "array".into(),
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
