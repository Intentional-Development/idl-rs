use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};
use std::{
    collections::{BTreeMap, HashSet},
    env, fs,
    path::PathBuf,
};

fn main() -> Result<()> {
    let args = Args::parse()?;
    let spec_text = fs::read_to_string(&args.spec)
        .with_context(|| format!("read OpenAPI spec {}", args.spec.display()))?;
    let spec: Value = serde_yaml::from_str(&spec_text).context("parse OpenAPI YAML")?;

    let base_text = fs::read_to_string(&args.base_graph)
        .with_context(|| format!("read base graph {}", args.base_graph.display()))?;
    let mut graph: Value = serde_json::from_str(&base_text).context("parse base graph JSON")?;

    graph["version"] = Value::String("0.1.9".into());
    graph
        .get_mut("metadata")
        .and_then(Value::as_object_mut)
        .context("graph.metadata must be an object")?
        .extend([
            (
                "extractor".into(),
                Value::String("idl-extractors/openapi_to_idl@0.1.9".into()),
            ),
            (
                "source_openapi".into(),
                Value::String(args.spec.display().to_string()),
            ),
        ]);

    let schemas = spec
        .pointer("/components/schemas")
        .and_then(Value::as_object)
        .context("OpenAPI spec missing components.schemas")?;

    let dto_ns = graph
        .get_mut("extensions")
        .and_then(Value::as_object_mut)
        .and_then(|ext| ext.get_mut("dto"))
        .and_then(Value::as_object_mut)
        .context("base graph missing extensions.dto")?;
    let existing_defs = dto_ns
        .get("definitions")
        .and_then(Value::as_array)
        .context("extensions.dto.definitions must be an array")?;

    let mut by_name: BTreeMap<String, Value> = BTreeMap::new();
    for def in existing_defs {
        if let Some(name) = def
            .get("id")
            .and_then(Value::as_str)
            .and_then(|id| id.strip_prefix("dto:"))
        {
            let mut def = def.clone();
            if def.get("kind").is_none() {
                def.as_object_mut()
                    .context("DTO definition must be an object")?
                    .insert("kind".into(), Value::String("object".into()));
            }
            by_name.insert(name.to_string(), def);
        }
    }

    // W22 TASK A: Extract nested schemas from paths (request/response bodies)
    let mut nested_count = 0;
    if let Some(paths) = spec.pointer("/paths").and_then(Value::as_object) {
        for (_path, methods) in paths {
            if let Some(methods_obj) = methods.as_object() {
                for (method, details) in methods_obj {
                    if !["get", "post", "put", "patch", "delete"].contains(&method.as_str()) {
                        continue;
                    }
                    if let Some(details_obj) = details.as_object() {
                        // Extract from requestBody
                        if let Some(content) = details_obj
                            .get("requestBody")
                            .and_then(|rb| rb.get("content"))
                            .and_then(Value::as_object)
                        {
                            for schema_info in content.values() {
                                if let Some(schema) = schema_info.get("schema") {
                                    if let Some((name, def)) =
                                        extract_nested_schema(schema, schemas)
                                    {
                                        if let std::collections::btree_map::Entry::Vacant(entry) =
                                            by_name.entry(name)
                                        {
                                            entry.insert(def);
                                            nested_count += 1;
                                        }
                                    }
                                }
                            }
                        }
                        // Extract from responses
                        if let Some(responses) =
                            details_obj.get("responses").and_then(Value::as_object)
                        {
                            for response in responses.values() {
                                if let Some(content) =
                                    response.get("content").and_then(Value::as_object)
                                {
                                    for schema_info in content.values() {
                                        if let Some(schema) = schema_info.get("schema") {
                                            if let Some((name, def)) =
                                                extract_nested_schema(schema, schemas)
                                            {
                                                if let std::collections::btree_map::Entry::Vacant(
                                                    entry,
                                                ) = by_name.entry(name)
                                                {
                                                    entry.insert(def);
                                                    nested_count += 1;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // W22 TASK B: Build operation usage map for behavior classification
    let operation_usage = build_operation_usage(&spec);

    let mut skipped: Vec<String> = Vec::new();
    for (name, schema) in schemas {
        if by_name.contains_key(name) {
            continue;
        }
        if let Some(def) = definition_for_schema(name, schema, &operation_usage) {
            by_name.insert(name.clone(), def);
        } else {
            skipped.push(name.clone());
        }
    }

    let defs: Vec<Value> = by_name.into_values().collect();
    dto_ns.insert("definitions".into(), Value::Array(defs));

    graph
        .get_mut("metadata")
        .and_then(Value::as_object_mut)
        .context("graph.metadata must be an object")?
        .extend([
            ("openapi_schema_count".into(), json!(schemas.len())),
            ("openapi_schema_skipped".into(), json!(skipped)),
            ("nested_schemas_extracted".into(), json!(nested_count)),
        ]);

    let pretty = serde_json::to_string_pretty(&graph).context("serialize output graph")?;
    fs::write(&args.out, pretty).with_context(|| format!("write {}", args.out.display()))?;
    Ok(())
}

struct Args {
    spec: PathBuf,
    base_graph: PathBuf,
    out: PathBuf,
}

impl Args {
    fn parse() -> Result<Self> {
        let mut spec = None;
        let mut base_graph = None;
        let mut out = None;
        let mut it = env::args().skip(1);
        while let Some(arg) = it.next() {
            let value = match arg.as_str() {
                "--spec" => &mut spec,
                "--base-graph" => &mut base_graph,
                "--out" => &mut out,
                other => bail!("unknown argument {other}; expected --spec, --base-graph, --out"),
            };
            *value = Some(PathBuf::from(
                it.next()
                    .with_context(|| format!("missing value for {arg}"))?,
            ));
        }
        Ok(Self {
            spec: spec.context("--spec is required")?,
            base_graph: base_graph.context("--base-graph is required")?,
            out: out.context("--out is required")?,
        })
    }
}

fn definition_for_schema(
    name: &str,
    schema: &Value,
    operation_usage: &OperationUsage,
) -> Option<Value> {
    // v0.1.7: paginated — check for paginated list pattern (data: array<T> + metadata)
    if let Some(props) = schema.get("properties").and_then(Value::as_object) {
        if let Some(data_prop) = props.get("data") {
            if data_prop.get("type").and_then(Value::as_str) == Some("array") {
                if let Some(items) = data_prop.get("items") {
                    if let Some(ref_val) = items.get("$ref").and_then(Value::as_str) {
                        if let Some(item_name) = ref_val.strip_prefix("#/components/schemas/") {
                            // Check if this looks like a paginated schema (has pagination metadata)
                            let has_pagination_metadata = props.contains_key("has_more")
                                || props.contains_key("url")
                                || props.contains_key("meta")
                                || props.contains_key("links")
                                || props.contains_key("total")
                                || props.contains_key("object");

                            if has_pagination_metadata {
                                let mut def =
                                    common_def(name, schema, "paginated", operation_usage);
                                def.insert(
                                    "items".into(),
                                    Value::String(format!("dto:{item_name}")),
                                );

                                // Extract pagination metadata fields
                                if props.contains_key("has_more") {
                                    def.insert(
                                        "has_more_field".into(),
                                        Value::String("has_more".into()),
                                    );
                                }
                                if props.contains_key("total") {
                                    def.insert("total_field".into(), Value::String("total".into()));
                                }

                                // Collect other metadata fields
                                let mut meta_fields = Map::new();
                                for (k, v) in props.iter() {
                                    if k != "data" {
                                        let type_str = if v.get("type").is_some() {
                                            v.get("type")
                                                .and_then(Value::as_str)
                                                .unwrap_or("string")
                                                .to_string()
                                        } else if v.get("$ref").is_some() {
                                            "object".to_string()
                                        } else {
                                            "string".to_string()
                                        };
                                        meta_fields.insert(k.clone(), Value::String(type_str));
                                    }
                                }
                                if !meta_fields.is_empty() {
                                    def.insert("meta_fields".into(), Value::Object(meta_fields));
                                }

                                return Some(Value::Object(def));
                            }
                        }
                    }
                }
            }
        }
    }

    // v0.1.6: array-alias — check if schema is `type: array` with `items: $ref`
    if schema.get("type").and_then(Value::as_str) == Some("array") {
        if let Some(items) = schema.get("items") {
            if let Some(ref_val) = items.get("$ref").and_then(Value::as_str) {
                if let Some(item_name) = ref_val.strip_prefix("#/components/schemas/") {
                    let mut def = common_def(name, schema, "array-alias", operation_usage);
                    def.insert("items".into(), Value::String(format!("dto:{item_name}")));
                    return Some(Value::Object(def));
                }
            }
        }
    }

    // W19: discriminator without oneOf — Azure OpenAI pattern
    // Schema has discriminator + mapping but no oneOf (variants use allOf to inherit)
    if let Some(disc) = extract_discriminator(schema) {
        // Even if mapping is empty or missing, we can still create a union if we have a discriminator
        let mapping = disc.get("mapping").and_then(Value::as_object);
        let mut variants = Vec::new();

        if let Some(mapping) = mapping {
            // Extract variants from discriminator mapping
            for (_key, target) in mapping {
                if let Some(dto_ref) = target.as_str() {
                    variants.push(json!({"ref": dto_ref}));
                }
            }
        }

        // If we have variants from the mapping, create a union
        if !variants.is_empty() {
            let mut def = common_def(name, schema, "union", operation_usage);
            def.insert("variants".into(), Value::Array(variants));
            def.insert("discriminator".into(), disc);
            return Some(Value::Object(def));
        }
    }

    // v0.1.6: union — check for oneOf/anyOf
    if let Some(one_of) = schema.get("oneOf").and_then(Value::as_array) {
        if let Some(variants) = extract_union_variants(one_of) {
            let mut def = common_def(name, schema, "union", operation_usage);
            def.insert("variants".into(), Value::Array(variants));
            if let Some(disc) = extract_discriminator(schema) {
                def.insert("discriminator".into(), disc);
            }
            return Some(Value::Object(def));
        }
    }
    if let Some(any_of) = schema.get("anyOf").and_then(Value::as_array) {
        if let Some(variants) = extract_union_variants(any_of) {
            let mut def = common_def(name, schema, "union", operation_usage);
            def.insert("variants".into(), Value::Array(variants));
            if let Some(disc) = extract_discriminator(schema) {
                def.insert("discriminator".into(), disc);
            }
            return Some(Value::Object(def));
        }
    }

    // enum
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        let mut strings = Vec::new();
        let mut nullable = schema
            .get("nullable")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        for v in values {
            match v {
                Value::String(s) => strings.push(Value::String(s.clone())),
                Value::Null => nullable = true,
                other => strings.push(Value::String(value_to_literal(other))),
            }
        }
        if strings.is_empty() {
            return None;
        }
        let mut def = common_def(name, schema, "enum", operation_usage);
        def.insert("values".into(), Value::Array(strings));
        def.insert(
            "value_type".into(),
            Value::String(
                schema
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("string")
                    .to_string(),
            ),
        );
        if nullable {
            def.insert("nullable".into(), Value::Bool(true));
        }
        return Some(Value::Object(def));
    }

    let props_empty = schema
        .get("properties")
        .and_then(Value::as_object)
        .map_or(true, Map::is_empty);
    if props_empty {
        if let Some(additional) = schema.get("additionalProperties") {
            let mut def = common_def(name, schema, "map", operation_usage);
            def.insert("key_type".into(), Value::String("string".into()));
            def.insert(
                "value_type".into(),
                Value::String(schema_value_type(additional)),
            );
            return Some(Value::Object(def));
        }

        let def = common_def(name, schema, "unit", operation_usage);
        return Some(Value::Object(def));
    }

    // Regular object with properties - create standard object definition
    let def = common_def(name, schema, "object", operation_usage);
    Some(Value::Object(def))
}

fn extract_union_variants(items: &[Value]) -> Option<Vec<Value>> {
    let mut variants = Vec::new();
    for item in items {
        if let Some(ref_val) = item.get("$ref").and_then(Value::as_str) {
            if let Some(variant_name) = ref_val.strip_prefix("#/components/schemas/") {
                variants.push(json!({"ref": format!("dto:{variant_name}")}));
            }
        } else if let Some(type_str) = item.get("type").and_then(Value::as_str) {
            // inline primitive types (e.g., oneOf: [type: boolean, type: string])
            variants.push(json!({"type": type_str}));
        }
    }
    if variants.is_empty() {
        None
    } else {
        Some(variants)
    }
}

fn extract_discriminator(schema: &Value) -> Option<Value> {
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

fn common_def(
    name: &str,
    schema: &Value,
    kind: &str,
    operation_usage: &OperationUsage,
) -> Map<String, Value> {
    let mut def = Map::new();
    def.insert("id".into(), Value::String(format!("dto:{name}")));
    def.insert("kind".into(), Value::String(kind.into()));
    def.insert("state".into(), Value::String("proposed".into()));
    def.insert(
        "created_by".into(),
        Value::String("brownfield-extractor".into()),
    );
    def.insert(
        "source_anchors".into(),
        json!([{ "uri": format!("repo://IDL/conformance/firefly-iii/canonical/openapi.yaml#/components/schemas/{name}") }]),
    );

    // W22 TASK B: Classify behavior based on heuristics
    let behavior = classify_behavior(name, schema, kind, operation_usage);

    def.insert(
        "confidence".into(),
        json!({
            "score": 1.0,
            "model": "idl-extractors/openapi_to_idl@0.1.9",
            "metadata": {
                "behavior": behavior
            }
        }),
    );
    def
}

fn schema_value_type(schema: &Value) -> String {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        if let Some(name) = reference.strip_prefix("#/components/schemas/") {
            return format!("dto:{name}");
        }
    }
    match schema
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("string")
    {
        "integer" => "integer".into(),
        "number" => "number".into(),
        "boolean" => "boolean".into(),
        _ => "string".into(),
    }
}

fn value_to_literal(value: &Value) -> String {
    match value {
        Value::Bool(v) => v.to_string(),
        Value::Number(v) => v.to_string(),
        Value::String(v) => v.clone(),
        other => other.to_string(),
    }
}

// W22 TASK A: Extract nested schema from path operation
fn extract_nested_schema(
    schema: &Value,
    all_schemas: &serde_json::Map<String, Value>,
) -> Option<(String, Value)> {
    // Check if this is a $ref to a schema
    if let Some(ref_str) = schema.get("$ref").and_then(Value::as_str) {
        if let Some(name) = ref_str.strip_prefix("#/components/schemas/") {
            // Check if this schema has discriminator (our target for W22 Task A)
            if let Some(target_schema) = all_schemas.get(name) {
                if has_discriminator_or_polymorphic(target_schema) {
                    // This is a nested discriminator - we want to extract it
                    if let Some(def) =
                        definition_for_schema(name, target_schema, &OperationUsage::default())
                    {
                        return Some((name.to_string(), def));
                    }
                }
            }
        }
    }

    // Check inline oneOf/anyOf/allOf
    for key in &["oneOf", "anyOf", "allOf"] {
        if schema.get(*key).is_some() && has_discriminator_or_polymorphic(schema) {
            // Generate a name for this inline schema
            // For now, skip inline schemas (they need parent context for naming)
        }
    }

    None
}

fn has_discriminator_or_polymorphic(schema: &Value) -> bool {
    schema.get("discriminator").is_some()
        || schema.get("oneOf").is_some()
        || schema.get("anyOf").is_some()
}

// W22 TASK B: Build operation usage map for behavior classification
#[derive(Default)]
struct OperationUsage {
    read_operations: HashSet<String>,
    write_operations: HashSet<String>,
    mutation_operations: HashSet<String>,
}

fn build_operation_usage(spec: &Value) -> OperationUsage {
    let mut usage = OperationUsage::default();

    if let Some(paths) = spec.pointer("/paths").and_then(Value::as_object) {
        for methods in paths.values() {
            if let Some(methods_obj) = methods.as_object() {
                for (method, details) in methods_obj {
                    let method_str = method.as_str().to_lowercase();

                    // Collect schema names from request/response bodies
                    let mut schemas_in_op = HashSet::new();

                    // From requestBody
                    if let Some(content) = details
                        .get("requestBody")
                        .and_then(|rb| rb.get("content"))
                        .and_then(Value::as_object)
                    {
                        for schema_info in content.values() {
                            if let Some(ref_str) = schema_info
                                .get("schema")
                                .and_then(|s| s.get("$ref"))
                                .and_then(Value::as_str)
                            {
                                if let Some(name) = ref_str.strip_prefix("#/components/schemas/") {
                                    schemas_in_op.insert(name.to_string());
                                }
                            }
                        }
                    }

                    // From responses
                    if let Some(responses) = details.get("responses").and_then(Value::as_object) {
                        for response in responses.values() {
                            if let Some(content) =
                                response.get("content").and_then(Value::as_object)
                            {
                                for schema_info in content.values() {
                                    if let Some(ref_str) = schema_info
                                        .get("schema")
                                        .and_then(|s| s.get("$ref"))
                                        .and_then(Value::as_str)
                                    {
                                        if let Some(name) =
                                            ref_str.strip_prefix("#/components/schemas/")
                                        {
                                            schemas_in_op.insert(name.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Categorize operations
                    for schema_name in schemas_in_op {
                        match method_str.as_str() {
                            "get" | "head" | "options" => {
                                usage.read_operations.insert(schema_name.clone());
                            }
                            "post" => {
                                usage.write_operations.insert(schema_name.clone());
                            }
                            "put" | "patch" | "delete" => {
                                usage.mutation_operations.insert(schema_name.clone());
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    usage
}

// W22 TASK B: Classify DTO behavior based on heuristics
fn classify_behavior(
    name: &str,
    schema: &Value,
    kind: &str,
    operation_usage: &OperationUsage,
) -> String {
    let name_lower = name.to_lowercase();

    // Heuristic 1: Paginated is always query-result
    if kind == "paginated" {
        return "query-result".to_string();
    }

    // Heuristic 2: Event pattern (past tense)
    if name_lower.ends_with("ed")
        || name_lower.ends_with("event")
        || name_lower.contains("event")
        || name_lower.ends_with("notification")
    {
        return "event".to_string();
    }

    // Heuristic 3: Command pattern (verb-name + write operation)
    if (name_lower.starts_with("create")
        || name_lower.starts_with("update")
        || name_lower.starts_with("delete")
        || name_lower.starts_with("store")
        || name_lower.starts_with("trigger")
        || name_lower.contains("request"))
        && operation_usage.write_operations.contains(name)
    {
        return "command".to_string();
    }

    // Heuristic 4: Entity pattern (has id + used in mutation operations)
    if let Some(props) = schema.get("properties").and_then(Value::as_object) {
        let has_id = props.contains_key("id")
            || props.contains_key("ID")
            || props.contains_key("uuid")
            || props.contains_key("identifier");

        if has_id && operation_usage.mutation_operations.contains(name) {
            return "entity".to_string();
        }

        // Heuristic 5: Value-object (immutable, no id, composite structure)
        if !has_id && props.len() >= 2 && !operation_usage.mutation_operations.contains(name) {
            // Check if it's a simple value object (Address, Money, etc.)
            if name_lower.contains("address")
                || name_lower.contains("amount")
                || name_lower.contains("money")
                || name_lower.contains("date")
                || name_lower.contains("period")
                || name_lower.contains("range")
            {
                return "value-object".to_string();
            }
        }
    }

    // Heuristic 6: Query-result pattern (list/array/collection response)
    if name_lower.contains("list")
        || name_lower.contains("array")
        || name_lower.contains("collection")
        || name_lower.ends_with("s") && operation_usage.read_operations.contains(name)
    {
        return "query-result".to_string();
    }

    // Default: dto-only
    "dto-only".to_string()
}
