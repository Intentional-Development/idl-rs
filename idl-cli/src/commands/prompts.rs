//! `idl prompts` — derive AI assistant instructions from a graph.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use idl_graph::{GraphDoc, NodeDoc};

pub fn run(graph_path: PathBuf, target: String, out_dir: PathBuf) -> Result<ExitCode> {
    let graph = GraphDoc::load(&graph_path)
        .with_context(|| format!("load graph {}", graph_path.display()))?;
    let summary = PromptSummary::from_graph(&graph);
    let targets = PromptTarget::expand(&target)?;

    for target in targets {
        let path = out_dir.join(target.path());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create output directory {}", parent.display()))?;
        }
        let content = target.render(&summary);
        std::fs::write(&path, content).with_context(|| format!("write {}", path.display()))?;
        println!("wrote {}", path.display());
    }

    Ok(ExitCode::SUCCESS)
}

#[derive(Debug, Clone, Copy)]
enum PromptTarget {
    Cursor,
    Copilot,
    Claude,
}

impl PromptTarget {
    fn expand(value: &str) -> Result<Vec<Self>> {
        match value {
            "cursor" => Ok(vec![Self::Cursor]),
            "copilot" => Ok(vec![Self::Copilot]),
            "claude" => Ok(vec![Self::Claude]),
            "all" => Ok(vec![Self::Cursor, Self::Copilot, Self::Claude]),
            other => Err(anyhow::anyhow!(
                "unknown prompt target `{}`; expected cursor, copilot, claude, or all",
                other
            )),
        }
    }

    fn path(self) -> &'static Path {
        match self {
            Self::Cursor => Path::new(".cursorrules"),
            Self::Copilot => Path::new(".github/copilot-instructions.md"),
            Self::Claude => Path::new("CLAUDE.md"),
        }
    }

    fn render(self, summary: &PromptSummary) -> String {
        match self {
            Self::Cursor => render_cursor(summary),
            Self::Copilot => render_markdown("GitHub Copilot", summary),
            Self::Claude => render_markdown("Claude Code", summary),
        }
    }
}

#[derive(Debug, Default)]
struct PromptSummary {
    entities: Vec<String>,
    dtos: Vec<String>,
    apis: Vec<String>,
    operations: Vec<String>,
    constraints: Vec<String>,
    conventions: Vec<String>,
}

impl PromptSummary {
    fn from_graph(graph: &GraphDoc) -> Self {
        let mut summary = Self::default();
        for node in &graph.nodes {
            match node.kind.as_str() {
                "entity" => {
                    summary.entities.push(display_name(node));
                    collect_dtos(node, &mut summary.dtos);
                }
                "api" => summary.apis.push(display_name(node)),
                "operation" => summary.operations.push(display_name(node)),
                "constraints" | "invariant" | "policy" | "rule" => {
                    summary.constraints.push(display_name(node));
                }
                "decision" => summary.conventions.push(display_name(node)),
                _ => collect_dtos(node, &mut summary.dtos),
            }
        }
        sort_dedup(&mut summary.entities);
        sort_dedup(&mut summary.dtos);
        sort_dedup(&mut summary.apis);
        sort_dedup(&mut summary.operations);
        sort_dedup(&mut summary.constraints);
        sort_dedup(&mut summary.conventions);
        summary
    }
}

fn display_name(node: &NodeDoc) -> String {
    node.props
        .get("name")
        .and_then(|v| v.as_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| node.id.clone())
}

fn collect_dtos(node: &NodeDoc, dtos: &mut Vec<String>) {
    if let Some(value) = node.props.get("dto") {
        push_dto_value(value, dtos);
    }
    if let Some(value) = node.props.get("dtos") {
        push_dto_value(value, dtos);
    }
    if let Some(value) = node.props.get("dto_props") {
        push_dto_value(value, dtos);
    }
}

fn push_dto_value(value: &serde_json::Value, dtos: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => dtos.push(s.clone()),
        serde_json::Value::Array(items) => {
            for item in items {
                push_dto_value(item, dtos);
            }
        }
        serde_json::Value::Object(map) => {
            if let Some(name) = map.get("name").and_then(|v| v.as_str()) {
                dtos.push(name.to_string());
            }
        }
        _ => {}
    }
}

fn sort_dedup(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

fn render_cursor(summary: &PromptSummary) -> String {
    format!(
        "# IDL-derived Cursor rules\n\n{}\n\nUse these rules when editing this repository:\n- Preserve the IDL semantic graph as the source of truth.\n- Keep generated code aligned with APIs, entities, operations, and constraints.\n- Do not introduce behavior that violates listed constraints or conventions.\n",
        render_sections(summary)
    )
}

fn render_markdown(name: &str, summary: &PromptSummary) -> String {
    format!(
        "# IDL-derived instructions for {name}\n\n{}\n\n## Working rules\n- Treat the IDL graph as authoritative project context.\n- Prefer changes that preserve named APIs, domain entities, and operations.\n- Check constraints and conventions before suggesting implementation changes.\n",
        render_sections(summary)
    )
}

fn render_sections(summary: &PromptSummary) -> String {
    let mut out = String::new();
    append_section(&mut out, "Entities", &summary.entities);
    append_section(&mut out, "Key DTOs", &summary.dtos);
    append_section(&mut out, "APIs", &summary.apis);
    append_section(&mut out, "Operations", &summary.operations);
    append_section(&mut out, "Constraints and policies", &summary.constraints);
    append_section(&mut out, "Conventions and decisions", &summary.conventions);
    out
}

fn append_section(out: &mut String, title: &str, values: &[String]) {
    out.push_str(&format!("## {title}\n"));
    if values.is_empty() {
        out.push_str("- None declared\n\n");
    } else {
        for value in values {
            out.push_str(&format!("- {value}\n"));
        }
        out.push('\n');
    }
}
