//! `idl drift` — graph-aware drift (P1.7).
//!
//! Two sub-modes:
//! * `idl drift graph <baseline.json> <current.json>` — compare two graph
//!   files, classify each event as breaking / additive / cosmetic.
//! * `idl drift code <graph.json> --source <code-path>` — re-anchor each
//!   node's `source_anchors` against one or more source roots and report
//!   `aligned` / `shifted` / `missing` / `new-in-code`.
//!
//! ## `--source` flag
//!
//! Wave 10: `--source` may be specified multiple times. Each value is either
//!
//! * `<path>` — anonymous root used as a fallback for `repo://*` URIs whose
//!   first segment doesn't match any explicit corpus mapping, and for bare
//!   relative paths.
//! * `<corpus>=<path>` — named corpus mapping. Any `repo://<corpus>/...` URI
//!   is routed under `<path>` (e.g. `--source n8n=/path/to/n8n
//!   --source IDL=/path/to/IDL`).
//!
//! ## Exit codes
//!
//! Both `idl drift graph` and `idl drift code` use the same exit-code contract:
//!
//! | code | meaning                                                              |
//! |-----:|----------------------------------------------------------------------|
//! |    0 | aligned / no drift detected                                          |
//! |    1 | breaking drift (`drift code`: any `missing`; `drift graph`: removed) |
//! |    2 | additive drift (`new-in-code` / nodes added in current)              |
//! |    3 | cosmetic drift (`shifted` only)                                      |
//!
//! Higher-severity codes win when multiple categories are present (1 > 2 > 3).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use idl_emitters::{EmitReport, GraphEmitter, PythonEmitter, RustEmitter, TypeScriptEmitter};
use idl_graph::{diff_against_sources, diff_graphs, GraphDoc};
use serde::Serialize;

#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    Human,
    Json,
    Markdown,
}

pub fn run_graph(baseline: PathBuf, current: PathBuf, format: OutputFormat) -> Result<ExitCode> {
    let base = GraphDoc::load(&baseline)
        .with_context(|| format!("load baseline graph {}", baseline.display()))?;
    let cur = GraphDoc::load(&current)
        .with_context(|| format!("load current graph {}", current.display()))?;
    let report = diff_graphs(
        baseline.display().to_string(),
        &base,
        current.display().to_string(),
        &cur,
    );
    match format {
        OutputFormat::Human => println!("{}", report.to_human()),
        OutputFormat::Json => println!("{}", report.to_json()),
        OutputFormat::Markdown => println!("{}", report.to_markdown()),
    }
    Ok(ExitCode::from(report.exit_code()))
}

/// Parse a single `--source` argument into `(corpus, path)`. An empty corpus
/// name means "default / fallback root". Accepts both `name=path` and
/// `path-only` forms.
pub fn parse_source_mapping(raw: &str) -> (String, PathBuf) {
    if let Some((name, path)) = raw.split_once('=') {
        // Heuristic: if `name` looks like a path (contains '/' or starts with
        // '.') treat the whole input as a bare path. Otherwise it's a
        // corpus=path mapping.
        if name.contains('/') || name.starts_with('.') {
            return (String::new(), PathBuf::from(raw));
        }
        return (name.to_string(), PathBuf::from(path));
    }
    (String::new(), PathBuf::from(raw))
}

pub fn run_code(
    graph_path: PathBuf,
    sources: Vec<PathBuf>,
    format: OutputFormat,
) -> Result<ExitCode> {
    // Back-compat path: PathBuf carries the raw string. Re-parse to allow
    // `name=path` form even though clap handed us `PathBuf`.
    let mappings: Vec<(String, PathBuf)> = sources
        .iter()
        .map(|p| parse_source_mapping(&p.to_string_lossy()))
        .collect();
    let graph = GraphDoc::load(&graph_path)
        .with_context(|| format!("load graph {}", graph_path.display()))?;
    let report = diff_against_sources(graph_path.display().to_string(), &graph, &mappings);
    match format {
        OutputFormat::Human => println!("{}", report.to_human()),
        OutputFormat::Json => println!("{}", report.to_json()),
        OutputFormat::Markdown => println!("{}", report.to_markdown()),
    }
    Ok(ExitCode::from(report.exit_code()))
}

#[derive(Debug, Serialize)]
struct GateReport {
    graph: String,
    generated_root: String,
    verdict: String,
    targets: Vec<TargetGateReport>,
}

#[derive(Debug, Serialize)]
struct TargetGateReport {
    target: String,
    verdict: String,
    files_checked: usize,
    drift: Vec<FileDrift>,
}

#[derive(Debug, Serialize)]
struct FileDrift {
    path: String,
    reason: String,
}

pub fn run_gate(
    workspace: PathBuf,
    graph_override: Option<PathBuf>,
    generated_override: Option<PathBuf>,
    target_overrides: Vec<String>,
    ctx: &crate::output::OutputContext,
) -> Result<ExitCode> {
    let workspace = workspace.canonicalize().unwrap_or(workspace);
    let graph_path = match graph_override {
        Some(path) => absolutize(&workspace, path),
        None => {
            detect_graph(&workspace).ok_or_else(|| anyhow!("no graph file found for drift gate"))?
        }
    };
    let generated_root = generated_override
        .map(|path| absolutize(&workspace, path))
        .unwrap_or_else(|| workspace.join("generated"));
    let graph = GraphDoc::load(&graph_path)
        .with_context(|| format!("load graph {}", graph_path.display()))?;
    let targets = if target_overrides.is_empty() {
        configured_targets(&graph)
    } else {
        target_overrides
    };

    let mut target_reports = Vec::new();
    for target in targets {
        let normalized = normalize_target(&target)?;
        let emitted = emit_target(&normalized, &graph)?;
        let baseline_dir = target_dir(&generated_root, &normalized);
        let target_report = compare_target(&normalized, &emitted, &baseline_dir);
        target_reports.push(target_report);
    }

    let drifted = target_reports
        .iter()
        .any(|target| target.verdict == "drifted");
    let report = GateReport {
        graph: graph_path.display().to_string(),
        generated_root: generated_root.display().to_string(),
        verdict: if drifted { "drifted" } else { "clean" }.to_string(),
        targets: target_reports,
    };

    write_gate_status(&workspace, &report)?;

    if ctx.json_mode {
        ctx.json(&report)?;
    } else {
        print_gate_human(&report, ctx);
    }

    Ok(ExitCode::from(if drifted { 1 } else { 0 }))
}

fn absolutize(workspace: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        workspace.join(path)
    }
}

fn detect_graph(workspace: &Path) -> Option<PathBuf> {
    [
        "idl.graph.json",
        "graph.json",
        "semantic-graph.json",
        "intent/idl.graph.json",
        "intent/semantic-graph.json",
        "extracted/semantic-graph.json",
        "intent/extracted/semantic-graph.json",
    ]
    .iter()
    .map(|p| workspace.join(p))
    .find(|p| p.is_file())
}

fn configured_targets(graph: &GraphDoc) -> Vec<String> {
    let from_metadata = graph
        .metadata
        .get("emit_targets")
        .or_else(|| graph.metadata.get("targets"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(ToString::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if from_metadata.is_empty() {
        vec!["typescript".into(), "python".into(), "rust".into()]
    } else {
        from_metadata
    }
}

fn normalize_target(target: &str) -> Result<String> {
    match target {
        "ts" | "typescript" => Ok("typescript".to_string()),
        "py" | "python" => Ok("python".to_string()),
        "rs" | "rust" => Ok("rust".to_string()),
        other => Err(anyhow!(
            "unsupported drift gate target {other:?}; supported: typescript, python, rust"
        )),
    }
}

fn emit_target(target: &str, graph: &GraphDoc) -> Result<EmitReport> {
    match target {
        "typescript" => TypeScriptEmitter.emit(graph),
        "python" => PythonEmitter.emit(graph),
        "rust" => RustEmitter.emit(graph),
        other => Err(anyhow!("unsupported drift gate target {other:?}")),
    }
}

fn target_dir(generated_root: &Path, target: &str) -> PathBuf {
    let direct = generated_root.join(target);
    if direct.is_dir() {
        direct
    } else {
        generated_root.join("code").join(target)
    }
}

fn compare_target(target: &str, emitted: &EmitReport, baseline_dir: &Path) -> TargetGateReport {
    let mut drift = Vec::new();
    for file in &emitted.files {
        let baseline = baseline_dir.join(&file.path);
        match std::fs::read_to_string(&baseline) {
            Ok(existing) if existing == file.content => {}
            Ok(_) => drift.push(FileDrift {
                path: file.path.display().to_string(),
                reason: "content differs".to_string(),
            }),
            Err(_) => drift.push(FileDrift {
                path: file.path.display().to_string(),
                reason: format!("missing from {}", baseline_dir.display()),
            }),
        }
    }

    TargetGateReport {
        target: target.to_string(),
        verdict: if drift.is_empty() { "clean" } else { "drifted" }.to_string(),
        files_checked: emitted.files.len(),
        drift,
    }
}

fn write_gate_status(workspace: &Path, report: &GateReport) -> Result<()> {
    let dot_idl = workspace.join(".idl");
    std::fs::create_dir_all(&dot_idl).with_context(|| format!("create {}", dot_idl.display()))?;
    let status = serde_json::json!({
        "timestamp": Utc::now().to_rfc3339(),
        "verdict": report.verdict,
        "targets": report.targets,
    });
    std::fs::write(
        dot_idl.join("drift-status.json"),
        serde_json::to_string_pretty(&status)? + "\n",
    )?;
    Ok(())
}

fn print_gate_human(report: &GateReport, ctx: &crate::output::OutputContext) {
    ctx.stdout("IDL drift gate");
    ctx.stdout(&format!("  graph: {}", report.graph));
    ctx.stdout(&format!("  generated root: {}", report.generated_root));
    ctx.stdout(&format!("  verdict: {}", report.verdict));
    for target in &report.targets {
        ctx.stdout(&format!(
            "  {}: {} ({} files checked)",
            target.target, target.verdict, target.files_checked
        ));
        for item in &target.drift {
            ctx.stdout(&format!("    - {}: {}", item.path, item.reason));
        }
    }
}
