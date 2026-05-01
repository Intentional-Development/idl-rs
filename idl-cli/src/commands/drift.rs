//! `idl drift` — graph-aware drift (P1.7).
//!
//! Two sub-modes:
//! * `idl drift graph <baseline.json> <current.json>` — compare two graph
//!   files, classify each event as breaking / additive / cosmetic.
//! * `idl drift code <graph.json> --source <code-path>` — re-anchor each
//!   node's source_anchors and report aligned / shifted / missing.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use idl_graph::{diff_against_source, diff_graphs, GraphDoc};

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

pub fn run_code(
    graph_path: PathBuf,
    source: PathBuf,
    format: OutputFormat,
) -> Result<ExitCode> {
    let graph = GraphDoc::load(&graph_path)
        .with_context(|| format!("load graph {}", graph_path.display()))?;
    let report = diff_against_source(graph_path.display().to_string(), &graph, &source);
    match format {
        OutputFormat::Human => println!("{}", report.to_human()),
        OutputFormat::Json => println!("{}", report.to_json()),
        OutputFormat::Markdown => println!("{}", report.to_markdown()),
    }
    Ok(ExitCode::from(report.exit_code()))
}
