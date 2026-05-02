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

    graph["version"] = Value::String("0.1.6".into());
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
