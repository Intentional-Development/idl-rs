//! Graph-driven code emitters (Wave 8 R3 / P1.4).
//!
//! These emitters consume a [`idl_graph::GraphDoc`] (the schema-shaped JSON
//! produced by extractors) and write target-language scaffolding to disk.
//! They are intentionally minimal — round-tripping the kernel constructs is
//! the design goal, not generating production-ready code.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use idl_graph::{GraphDoc, NodeDoc};

pub mod openapi;
pub mod rust;
pub mod typescript;

pub use openapi::OpenApiEmitter;
pub use rust::RustEmitter;
pub use typescript::TypeScriptEmitter;

/// One emitted file.
#[derive(Debug, Clone)]
pub struct EmittedFile {
    pub path: PathBuf,
    pub content: String,
}

impl EmittedFile {
    pub fn loc(&self) -> usize {
        self.content.lines().count()
    }
}

/// Aggregate emit result.
#[derive(Debug, Default, Clone)]
pub struct EmitReport {
    pub target: String,
    pub files: Vec<EmittedFile>,
    pub nodes_emitted: usize,
}

impl EmitReport {
    pub fn total_loc(&self) -> usize {
        self.files.iter().map(|f| f.loc()).sum()
    }
    pub fn file_count(&self) -> usize {
        self.files.len()
    }
    pub fn write(&self, out: &Path) -> Result<()> {
        for f in &self.files {
            let abs = out.join(&f.path);
            if let Some(parent) = abs.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("create dir {}", parent.display()))?;
            }
            std::fs::write(&abs, &f.content)
                .with_context(|| format!("write {}", abs.display()))?;
        }
        Ok(())
    }
}

/// Graph-driven code emitter.
pub trait GraphEmitter {
    fn target(&self) -> &str;
    fn emit(&self, graph: &GraphDoc) -> Result<EmitReport>;
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

pub(crate) fn pascal_case(s: &str) -> String {
    let mut out = String::new();
    let mut upper = true;
    for c in s.chars() {
        if c == '_' || c == '-' || c == ' ' || c == ':' || c == '/' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

pub(crate) fn snake_case(s: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for c in s.chars() {
        if c == '-' || c == ' ' || c == ':' || c == '/' {
            out.push('_');
            prev_lower = false;
        } else if c.is_uppercase() {
            if prev_lower {
                out.push('_');
            }
            out.extend(c.to_lowercase());
            prev_lower = false;
        } else {
            out.push(c);
            prev_lower = c.is_alphanumeric();
        }
    }
    out
}

pub(crate) fn safe_ident(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    if out.chars().next().map_or(true, |c| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

pub(crate) fn node_name(node: &NodeDoc) -> String {
    node.props
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            node.id
                .rsplit_once(':')
                .map(|(_, s)| s.to_string())
                .unwrap_or_else(|| node.id.clone())
        })
}
