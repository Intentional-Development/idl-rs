//! `idl validate` (P1.8).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use idl_graph::{default_constraints, Severity, ValidationReport};
use serde::Serialize;

use crate::{diagnostic_formatter::format_message_with_dtos, exit_codes, graph_build::lift_document, output};

#[derive(Serialize)]
struct ValidateOutput<'a> {
    source: String,
    strict: bool,
    coverage_pct: f32,
    recognized_blocks: usize,
    total_blocks: usize,
    lost_blocks: usize,
    report: &'a ValidationReport,
}

pub fn run(path: Option<PathBuf>, strict: bool, ctx: &output::OutputContext) -> Result<ExitCode> {
    let path = resolve_path(path)?;
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("read IDL file {}", path.display()))?;
    let doc =
        idl_core::parse_idl(&text).with_context(|| format!("parse IDL file {}", path.display()))?;

    let lifted = lift_document(&doc, &path.display().to_string());
    let constraints = default_constraints();
    let mut report = lifted.graph.validate(&constraints);

    // In strict mode, any recorded loss is an error and warnings escalate.
    if strict {
        if !lifted.loss.lost_blocks.is_empty() {
            for lost in &lifted.loss.lost_blocks {
                report.errors.push(idl_graph::ConstraintViolation {
                    rule: "kernel-only-strict".into(),
                    severity: Severity::Error,
                    message: format!(
                        "non-kernel block `{}` rejected in --strict mode",
                        lost.block_kind
                    ),
                    node: None,
                    edge: None,
                });
            }
        }
        let escalated: Vec<_> = report.warnings.drain(..).collect();
        for mut w in escalated {
            w.severity = Severity::Error;
            report.errors.push(w);
        }
    }

    let exit = if !report.errors.is_empty() {
        exit_codes::error()
    } else if !report.warnings.is_empty() {
        ExitCode::from(2) // warnings as distinct code
    } else {
        exit_codes::success()
    };

    if ctx.json_mode {
        let out = ValidateOutput {
            source: path.display().to_string(),
            strict,
            coverage_pct: lifted.loss.coverage_pct(),
            recognized_blocks: lifted.loss.recognized_blocks,
            total_blocks: lifted.loss.total_blocks,
            lost_blocks: lifted.loss.lost_blocks.len(),
            report: &report,
        };
        ctx.json(&out)?;
    } else {
        print_human(&path, strict, &lifted.loss, &report, ctx);
    }

    Ok(exit)
}

fn resolve_path(path: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = path {
        return Ok(p);
    }
    let candidates = [
        PathBuf::from("intent/project.idl"),
        PathBuf::from("project.idl"),
    ];
    for c in &candidates {
        if c.is_file() {
            return Ok(c.clone());
        }
    }
    anyhow::bail!(
        "no IDL file found (looked for intent/project.idl and project.idl); pass a path explicitly"
    )
}

fn print_human(
    path: &Path,
    strict: bool,
    loss: &idl_graph::SemanticLossReport,
    report: &ValidationReport,
    ctx: &output::OutputContext,
) {
    ctx.info(&format!("idl validate{}", if strict { " --strict" } else { "" }));
    ctx.info(&format!("  source: {}", path.display()));
    ctx.info(&format!(
        "  coverage: {:.1}% ({}/{} blocks recognized, {} lost)",
        loss.coverage_pct(),
        loss.recognized_blocks,
        loss.total_blocks,
        loss.lost_blocks.len()
    ));
    ctx.info(&format!("  constraints checked: {}", report.checked.len()));
    ctx.info(&format!(
        "  errors: {}, warnings: {}, infos: {}",
        report.errors.len(),
        report.warnings.len(),
        report.infos.len()
    ));

    let dtos = vec![];

    for e in &report.errors {
        let formatted_msg = format_message_with_dtos(&e.message, &dtos);
        ctx.error(&format!("[{}] {}", e.rule, formatted_msg));
    }
    for w in &report.warnings {
        let formatted_msg = format_message_with_dtos(&w.message, &dtos);
        ctx.warn(&format!("[{}] {}", w.rule, formatted_msg));
    }
    for i in &report.infos {
        let formatted_msg = format_message_with_dtos(&i.message, &dtos);
        ctx.info(&format!("INFO   [{}] {}", i.rule, formatted_msg));
    }

    if !loss.lost_blocks.is_empty() {
        ctx.info("  lost blocks:");
        for l in &loss.lost_blocks {
            ctx.info(&format!("    - {} ({})", l.block_kind, l.reason.as_str()));
        }
    }
}
