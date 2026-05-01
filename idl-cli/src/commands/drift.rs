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

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use idl_graph::{diff_against_sources, diff_graphs, GraphDoc};

#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    Human,
    Json,
    Markdown,
}

pub fn run_graph(
    baseline: PathBuf,
    current: PathBuf,
    format: OutputFormat,
) -> Result<ExitCode> {
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
