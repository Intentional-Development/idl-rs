//! `idl emit <target> <graph.json> --out <dir>` — graph-driven codegen
//! (Wave 8 R3 / P1.4). Targets: `rust`, `typescript`, `python`, `openapi`.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{anyhow, Context, Result};
use idl_emitters::{GraphEmitter, OpenApiEmitter, PythonEmitter, RustEmitter, TypeScriptEmitter};
use idl_graph::GraphDoc;

pub fn run(target: String, graph_path: PathBuf, out: PathBuf) -> Result<ExitCode> {
    let graph = GraphDoc::load(&graph_path)
        .with_context(|| format!("load graph {}", graph_path.display()))?;

    let report = match target.as_str() {
        "rust" => RustEmitter.emit(&graph)?,
        "python" | "py" => PythonEmitter.emit(&graph)?,
        "typescript" | "ts" => TypeScriptEmitter.emit(&graph)?,
        "openapi" => OpenApiEmitter.emit(&graph)?,
        other => {
            return Err(anyhow!(
                "unknown emit target {other:?}; supported: rust, python, typescript, openapi"
            ));
        }
    };

    std::fs::create_dir_all(&out).with_context(|| format!("create out dir {}", out.display()))?;
    report.write(&out)?;

    println!(
        "idl emit {target}: wrote {} files ({} LOC) covering {} nodes → {}",
        report.file_count(),
        report.total_loc(),
        report.nodes_emitted,
        out.display()
    );
    Ok(ExitCode::from(0))
}
