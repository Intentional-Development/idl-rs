use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};
use std::{collections::BTreeMap, env, fs, path::PathBuf};

fn main() -> Result<()> {
    let args = Args::parse()?;
    let spec_text = fs::read_to_string(&args.spec)
        .with_context(|| format!("read OpenAPI spec {}", args.spec.display()))?;
    let spec: Value = serde_yaml::from_str(&spec_text).context("parse OpenAPI YAML")?;

    let base_text = fs::read_to_string(&args.base_graph)
        .with_context(|| format!("read base graph {}", args.base_graph.display()))?;
    let mut graph: Value = serde_json::from_str(&base_text).context("parse base graph JSON")?;

    graph["version"] = Value::String("0.1.7".into());
    graph
        .get_mut("metadata")
        .and_then(Value::as_object_mut)
        .context("graph.metadata must be an object")?
        .extend([
            (
                "extractor".into(),
                Value::String("idl-extractors/openapi_to_idl@0.1.6".into()),
            ),
            ("source_openapi".into(), Value::String(args.spec.display().to_string())),
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

    let mut skipped: Vec<String> = Vec::new();
    for (name, schema) in schemas {
        if by_name.contains_key(name) {
            continue;
        }
        if let Some(def) = definition_for_schema(name, schema) {
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
                it.next().with_context(|| format!("missing value for {arg}"))?,
            ));
        }
        Ok(Self {
            spec: spec.context("--spec is required")?,
            base_graph: base_graph.context("--base-graph is required")?,
            out: out.context("--out is required")?,
        })
    }
}

fn definition_for_schema(name: &str, schema: &Value) -> Option<Value> {
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
                                let mut def = common_def(name, schema, "paginated");
                                def.insert("items".into(), Value::String(format!("dto:{item_name}")));
                                
                                // Extract pagination metadata fields
                                if props.contains_key("has_more") {
                                    def.insert("has_more_field".into(), Value::String("has_more".into()));
                                }
                                if props.contains_key("total") {
                                    def.insert("total_field".into(), Value::String("total".into()));
                                }
                                
                                // Collect other metadata fields
                                let mut meta_fields = Map::new();
                                for (k, v) in props.iter() {
                                    if k != "data" {
                                        let type_str = if v.get("type").is_some() {
                                            v.get("type").and_then(Value::as_str).unwrap_or("string").to_string()
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
                    let mut def = common_def(name, schema, "array-alias");
                    def.insert("items".into(), Value::String(format!("dto:{item_name}")));
                    return Some(Value::Object(def));
                }
            }
        }
    }

    // v0.1.6: union — check for oneOf/anyOf
    if let Some(one_of) = schema.get("oneOf").and_then(Value::as_array) {
        if let Some(variants) = extract_union_variants(one_of) {
            let mut def = common_def(name, schema, "union");
            def.insert("variants".into(), Value::Array(variants));
            if let Some(disc) = extract_discriminator(schema) {
                def.insert("discriminator".into(), disc);
            }
            return Some(Value::Object(def));
        }
    }
    if let Some(any_of) = schema.get("anyOf").and_then(Value::as_array) {
        if let Some(variants) = extract_union_variants(any_of) {
            let mut def = common_def(name, schema, "union");
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
        let mut nullable = schema.get("nullable").and_then(Value::as_bool).unwrap_or(false);
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
        let mut def = common_def(name, schema, "enum");
        def.insert("values".into(), Value::Array(strings));
        def.insert(
            "value_type".into(),
            Value::String(schema.get("type").and_then(Value::as_str).unwrap_or("string").to_string()),
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
            let mut def = common_def(name, schema, "map");
            def.insert("key_type".into(), Value::String("string".into()));
            def.insert("value_type".into(), Value::String(schema_value_type(additional)));
            return Some(Value::Object(def));
        }

        let def = common_def(name, schema, "unit");
        return Some(Value::Object(def));
    }

    None
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

fn common_def(name: &str, _schema: &Value, kind: &str) -> Map<String, Value> {
    let mut def = Map::new();
    def.insert("id".into(), Value::String(format!("dto:{name}")));
    def.insert("kind".into(), Value::String(kind.into()));
    def.insert("state".into(), Value::String("proposed".into()));
    def.insert("created_by".into(), Value::String("brownfield-extractor".into()));
    def.insert(
        "source_anchors".into(),
        json!([{ "uri": format!("repo://IDL/conformance/firefly-iii/canonical/openapi.yaml#/components/schemas/{name}") }]),
    );
    def.insert(
        "confidence".into(),
        json!({ "score": 1.0, "model": "idl-extractors/openapi_to_idl" }),
    );
    def
}

fn schema_value_type(schema: &Value) -> String {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        if let Some(name) = reference.strip_prefix("#/components/schemas/") {
            return format!("dto:{name}");
        }
    }
    match schema.get("type").and_then(Value::as_str).unwrap_or("string") {
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
