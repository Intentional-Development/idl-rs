//! `idl status` — workspace health summary for graph, schema, proposals, drift, and conformance.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use idl_graph::GraphDoc;
use serde::Serialize;
use serde_json::{json, Value};

use crate::{exit_codes, output};
use idl_proposals::{list_proposals, ProposalStatus};

const EMBEDDED_SCHEMA: &str = include_str!("../../../../IDL/schemas/semantic-graph.schema.json");

#[derive(Debug, Serialize)]
struct StatusReport {
    workspace: WorkspaceStatus,
    schema: SchemaStatus,
    proposals: ProposalCounts,
    drift: DriftStatus,
    conformance: Vec<ConformanceStatus>,
    warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
struct WorkspaceStatus {
    cwd: String,
    graph_path: String,
    schema_version: String,
}

#[derive(Debug, Serialize)]
struct SchemaStatus {
    declared_version: String,
    installed_version: String,
    matches: bool,
}

#[derive(Debug, Default, Serialize)]
struct ProposalCounts {
    pending: usize,
    accepted: usize,
    rejected: usize,
}

#[derive(Debug, Serialize)]
struct DriftStatus {
    last_run_timestamp: Option<String>,
    last_verdict: String,
}

#[derive(Debug, Serialize)]
struct ConformanceStatus {
    corpus: String,
    summary: String,
}

pub fn run(graph: Option<PathBuf>, ctx: &output::OutputContext) -> Result<ExitCode> {
    let cwd = std::env::current_dir().context("resolve current directory")?;
    let graph_path = match graph {
        Some(path) => path,
        None => detect_graph(&cwd).ok_or_else(|| {
            anyhow::anyhow!(
                "no graph file found (looked for idl.graph.json, graph.json, semantic-graph.json, intent/idl.graph.json, intent/semantic-graph.json, extracted/semantic-graph.json, intent/extracted/semantic-graph.json)"
            )
        })?,
    };

    if !graph_path.exists() {
        bail!("graph file {} does not exist", graph_path.display());
    }

    let graph_doc = GraphDoc::load(&graph_path)
        .with_context(|| format!("load graph {}", graph_path.display()))?;
    let declared = normalize_version(&graph_doc.version);
    let installed = installed_schema_version();
    let matches = declared == normalize_version(&installed);
    let mut warnings = Vec::new();
    if !matches {
        warnings.push(format!(
            "schema version mismatch: graph declares {}, installed schema is {}",
            graph_doc.version, installed
        ));
    }

    let report = StatusReport {
        workspace: WorkspaceStatus {
            cwd: cwd.display().to_string(),
            graph_path: graph_path.display().to_string(),
            schema_version: graph_doc.version.clone(),
        },
        schema: SchemaStatus {
            declared_version: graph_doc.version.clone(),
            installed_version: installed,
            matches,
        },
        proposals: proposal_counts(),
        drift: read_drift_status(&cwd),
        conformance: read_conformance(&cwd, &graph_doc),
        warnings,
    };

    if ctx.json_mode {
        ctx.json(&report)?;
    } else {
        print_human(&report, ctx);
    }

    Ok(exit_codes::success())
}

fn detect_graph(cwd: &Path) -> Option<PathBuf> {
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
    .map(|p| cwd.join(p))
    .find(|p| p.is_file())
}

fn installed_schema_version() -> String {
    let schema: Value = serde_json::from_str(EMBEDDED_SCHEMA).unwrap_or_else(|_| json!({}));
    schema
        .get("$id")
        .and_then(|v| v.as_str())
        .and_then(|id| id.rsplit('/').next())
        .map(|s| s.trim_start_matches('v').to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn normalize_version(version: &str) -> String {
    version.trim().trim_start_matches('v').to_string()
}

fn proposal_counts() -> ProposalCounts {
    let mut counts = ProposalCounts::default();
    if let Ok(proposals) = list_proposals(None) {
        for (_, proposal) in proposals {
            match proposal.status {
                ProposalStatus::Pending => counts.pending += 1,
                ProposalStatus::Accepted => counts.accepted += 1,
                ProposalStatus::Rejected => counts.rejected += 1,
            }
        }
    }
    counts
}

fn read_drift_status(cwd: &Path) -> DriftStatus {
    let candidates = [
        cwd.join(".idl/drift-status.json"),
        cwd.join("intent/.idl/drift-status.json"),
    ];
    for candidate in candidates {
        if let Ok(text) = std::fs::read_to_string(&candidate) {
            if let Ok(value) = serde_json::from_str::<Value>(&text) {
                let ts = value
                    .get("timestamp")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let verdict = value
                    .get("verdict")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                return DriftStatus {
                    last_run_timestamp: ts,
                    last_verdict: verdict,
                };
            }
        }
    }
    DriftStatus {
        last_run_timestamp: None,
        last_verdict: "unknown".to_string(),
    }
}

fn read_conformance(cwd: &Path, graph: &GraphDoc) -> Vec<ConformanceStatus> {
    let mut entries = BTreeMap::new();

    if let Some(corpora) = graph
        .metadata
        .get("conformance")
        .and_then(|v| v.get("corpora"))
        .and_then(|v| v.as_array())
    {
        for corpus in corpora.iter().filter_map(|v| v.as_str()) {
            entries.insert(corpus.to_string(), "linked in graph metadata".to_string());
        }
    }

    for root in [cwd.join("conformance"), cwd.join("IDL/conformance")] {
        if let Ok(read_dir) = std::fs::read_dir(root) {
            for entry in read_dir.flatten() {
                let path = entry.path().join("conformance-report.json");
                if !path.is_file() {
                    continue;
                }
                if let Ok(text) = std::fs::read_to_string(&path) {
                    if let Ok(value) = serde_json::from_str::<Value>(&text) {
                        let corpus = value
                            .get("corpus")
                            .and_then(|v| v.as_str())
                            .map(String::from)
                            .or_else(|| entry.file_name().to_str().map(String::from))
                            .unwrap_or_else(|| "unknown".to_string());
                        entries.insert(corpus, summarize_conformance(&value));
                    }
                }
            }
        }
    }

    entries
        .into_iter()
        .map(|(corpus, summary)| ConformanceStatus { corpus, summary })
        .collect()
}

fn summarize_conformance(value: &Value) -> String {
    if let Some(summary) = value.get("summary").and_then(|v| v.as_str()) {
        return summary.to_string();
    }
    if let Some(status) = value.get("status").and_then(|v| v.as_str()) {
        return status.to_string();
    }
    let endpoints = value
        .pointer("/endpoints/present_pct")
        .and_then(|v| v.as_f64());
    let schemas = value
        .pointer("/schemas/strict_pct")
        .and_then(|v| v.as_f64());
    match (endpoints, schemas) {
        (Some(e), Some(s)) => format!("endpoints {:.1}%, schemas {:.1}%", e, s),
        (Some(e), None) => format!("endpoints {:.1}%", e),
        _ => "report found".to_string(),
    }
}

fn print_human(report: &StatusReport, ctx: &output::OutputContext) {
    ctx.stdout("IDL workspace status");
    ctx.stdout("\nWorkspace");
    ctx.stdout(&format!("  cwd: {}", report.workspace.cwd));
    ctx.stdout(&format!("  graph: {}", report.workspace.graph_path));
    ctx.stdout(&format!("  schema version: {}", report.workspace.schema_version));

    ctx.stdout("\nSchema");
    ctx.stdout(&format!("  declared: {}", report.schema.declared_version));
    ctx.stdout(&format!("  installed: {}", report.schema.installed_version));
    ctx.stdout(&format!(
        "  match: {}",
        if report.schema.matches { "yes" } else { "no" }
    ));

    ctx.stdout("\nProposals");
    ctx.stdout(&format!("  pending: {}", report.proposals.pending));
    ctx.stdout(&format!("  accepted: {}", report.proposals.accepted));
    ctx.stdout(&format!("  rejected: {}", report.proposals.rejected));

    ctx.stdout("\nDrift");
    ctx.stdout(&format!(
        "  last run: {}",
        report
            .drift
            .last_run_timestamp
            .as_deref()
            .unwrap_or("unknown")
    ));
    ctx.stdout(&format!("  last verdict: {}", report.drift.last_verdict));

    ctx.stdout("\nConformance");
    if report.conformance.is_empty() {
        ctx.stdout("  none linked");
    } else {
        for item in &report.conformance {
            ctx.stdout(&format!("  {}: {}", item.corpus, item.summary));
        }
    }

    if !report.warnings.is_empty() {
        ctx.stdout("\nWarnings");
        for warning in &report.warnings {
            ctx.warn(warning);
        }
    }
}
