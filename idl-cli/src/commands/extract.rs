//! `idl extract` — scaffold for brownfield extraction adapters.
//!
//! Wave 8 R4 adds the `--rewrite-anchors <old-prefix> <new-prefix>` helper
//! which edits `source_anchors[].uri` prefixes across a graph file. This
//! is used to migrate legacy graphs to the canonical
//! `repo://<corpus>/<path>` convention (see IDL/docs/idl-format-reference.md
//! §5.1).

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{anyhow, Context, Result};

pub fn run(
    source: Option<PathBuf>,
    output: Option<PathBuf>,
    language: Option<String>,
    rewrite_anchors: Option<Vec<String>>,
    in_place: bool,
) -> Result<ExitCode> {
    if let Some(prefixes) = rewrite_anchors {
        let old = prefixes
            .first()
            .ok_or_else(|| anyhow!("--rewrite-anchors requires <old> <new>"))?;
        let new = prefixes
            .get(1)
            .ok_or_else(|| anyhow!("--rewrite-anchors requires <old> <new>"))?;
        let graph_path = output.clone().or_else(|| source.clone()).ok_or_else(|| {
            anyhow!("pass --output <graph.json> (or --source) for rewrite-anchors")
        })?;
        return rewrite_anchors_in_file(&graph_path, old, new, in_place);
    }

    println!(
        "TODO: extraction adapters land in next pass; see brownfield extraction docs.\n  source:   {}\n  output:   {}\n  language: {}",
        source.as_ref().map(|p| p.display().to_string()).unwrap_or("(unset)".into()),
        output.as_ref().map(|p| p.display().to_string()).unwrap_or("(unset)".into()),
        language.as_deref().unwrap_or("(unset)"),
    );
    Ok(ExitCode::from(0))
}

pub fn rewrite_anchors_in_file(
    graph_path: &std::path::Path,
    old_prefix: &str,
    new_prefix: &str,
    in_place: bool,
) -> Result<ExitCode> {
    let text = std::fs::read_to_string(graph_path)
        .with_context(|| format!("read {}", graph_path.display()))?;
    let mut value: serde_json::Value = serde_json::from_str(&text)
        .with_context(|| format!("parse {} as JSON", graph_path.display()))?;
    let rewritten = rewrite_anchors_in_value(&mut value, old_prefix, new_prefix);
    let pretty = serde_json::to_string_pretty(&value)?;

    let target = if in_place {
        // Preserve original to *.legacy.json next to it.
        let legacy = sibling_with_suffix(graph_path, "legacy.json");
        std::fs::write(&legacy, &text)
            .with_context(|| format!("write legacy {}", legacy.display()))?;
        graph_path.to_path_buf()
    } else {
        sibling_with_suffix(graph_path, "rewritten.json")
    };
    std::fs::write(&target, pretty).with_context(|| format!("write {}", target.display()))?;

    println!(
        "rewrote {rewritten} anchors: `{old_prefix}` -> `{new_prefix}`\n  input:  {}\n  output: {}",
        graph_path.display(),
        target.display()
    );
    Ok(ExitCode::from(0))
}

fn sibling_with_suffix(path: &std::path::Path, suffix: &str) -> PathBuf {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "graph".into());
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    parent.join(format!("{stem}.{suffix}"))
}

/// Rewrite anchor URIs inside a parsed graph document. Returns count of
/// anchors mutated.
pub fn rewrite_anchors_in_value(
    value: &mut serde_json::Value,
    old_prefix: &str,
    new_prefix: &str,
) -> usize {
    let mut count = 0;
    let Some(nodes) = value.get_mut("nodes").and_then(|n| n.as_array_mut()) else {
        return 0;
    };
    for node in nodes {
        let Some(anchors) = node
            .get_mut("source_anchors")
            .and_then(|n| n.as_array_mut())
        else {
            continue;
        };
        for anchor in anchors {
            let Some(uri) = anchor
                .get_mut("uri")
                .and_then(|u| u.as_str().map(String::from))
            else {
                continue;
            };
            if let Some(rest) = uri.strip_prefix(old_prefix) {
                let new_uri = format!("{new_prefix}{rest}");
                anchor["uri"] = serde_json::Value::String(new_uri);
                count += 1;
            }
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rewrites_matching_prefixes() {
        let mut v = json!({
            "version": "0.1.0",
            "nodes": [
                {
                    "id": "n:1",
                    "kind": "entity",
                    "state": "accepted",
                    "props": {},
                    "source_anchors": [
                        {"uri": "repo://packages/cli/src/foo.ts"},
                        {"uri": "repo://packages/core/src/bar.ts"}
                    ]
                },
                {
                    "id": "n:2",
                    "kind": "operation",
                    "state": "proposed",
                    "props": {},
                    "source_anchors": [
                        {"uri": "repo://other/path.ts"}
                    ]
                }
            ],
            "edges": []
        });
        let n = rewrite_anchors_in_value(&mut v, "repo://packages/", "repo://n8n/packages/");
        assert_eq!(n, 2);
        let uris: Vec<String> = v["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|n| n["source_anchors"].as_array().unwrap().iter())
            .map(|a| a["uri"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(uris[0], "repo://n8n/packages/cli/src/foo.ts");
        assert_eq!(uris[1], "repo://n8n/packages/core/src/bar.ts");
        assert_eq!(uris[2], "repo://other/path.ts");
    }
}
