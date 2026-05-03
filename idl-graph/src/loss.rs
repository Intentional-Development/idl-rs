//! Semantic loss reporting (P0.7).
//!
//! A [`SemanticLossReport`] captures the IDL blocks that could not be losslessly
//! lifted into the property graph — either because they used unknown constructs,
//! failed to parse, violated the schema, or required a disabled extension.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

/// Why a single IDL block was not represented in the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LossReason {
    UnknownConstruct,
    ParseError,
    SchemaViolation,
    ExtensionNotEnabled,
}

impl LossReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            LossReason::UnknownConstruct => "unknown_construct",
            LossReason::ParseError => "parse_error",
            LossReason::SchemaViolation => "schema_violation",
            LossReason::ExtensionNotEnabled => "extension_not_enabled",
        }
    }
}

/// One IDL block lost / degraded during graph construction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LossEntry {
    pub block_kind: String,
    pub line_range: (usize, usize),
    pub reason: LossReason,
    pub raw_excerpt: String,
}

/// Aggregate semantic-loss report for a single source artifact.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticLossReport {
    pub source_path: String,
    pub lost_blocks: Vec<LossEntry>,
    pub total_blocks: usize,
    pub recognized_blocks: usize,
}

impl SemanticLossReport {
    pub fn new(source_path: impl Into<String>) -> Self {
        Self {
            source_path: source_path.into(),
            ..Self::default()
        }
    }

    /// Coverage percentage in `[0.0, 100.0]`. Empty inputs are reported as
    /// 100% (no blocks => no loss).
    pub fn coverage_pct(&self) -> f32 {
        if self.total_blocks == 0 {
            return 100.0;
        }
        (self.recognized_blocks as f32 / self.total_blocks as f32) * 100.0
    }

    /// Render a short Markdown summary suitable for CI logs / PR comments.
    pub fn render_markdown(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "# Semantic loss report");
        let _ = writeln!(s, "- **source**: `{}`", self.source_path);
        let _ = writeln!(
            s,
            "- **coverage**: {:.1}% ({}/{} blocks recognized)",
            self.coverage_pct(),
            self.recognized_blocks,
            self.total_blocks
        );
        let _ = writeln!(s, "- **lost blocks**: {}", self.lost_blocks.len());
        if self.lost_blocks.is_empty() {
            let _ = writeln!(s, "\n_No semantic loss detected._");
            return s;
        }
        let _ = writeln!(s);
        let _ = writeln!(s, "| block | lines | reason | excerpt |");
        let _ = writeln!(s, "|-------|-------|--------|---------|");
        for entry in &self.lost_blocks {
            let excerpt = entry.raw_excerpt.replace('|', "\\|").replace('\n', " ⏎ ");
            let excerpt = if excerpt.len() > 80 {
                format!("{}…", &excerpt[..80])
            } else {
                excerpt
            };
            let _ = writeln!(
                s,
                "| `{}` | {}–{} | `{}` | {} |",
                entry.block_kind,
                entry.line_range.0,
                entry.line_range.1,
                entry.reason.as_str(),
                excerpt
            );
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_pct_handles_empty() {
        let r = SemanticLossReport::new("foo.idl");
        assert_eq!(r.coverage_pct(), 100.0);
    }

    #[test]
    fn coverage_pct_partial() {
        let mut r = SemanticLossReport::new("foo.idl");
        r.total_blocks = 10;
        r.recognized_blocks = 7;
        assert!((r.coverage_pct() - 70.0).abs() < 1e-3);
    }

    #[test]
    fn markdown_lists_lost_blocks() {
        let mut r = SemanticLossReport::new("foo.idl");
        r.total_blocks = 2;
        r.recognized_blocks = 1;
        r.lost_blocks.push(LossEntry {
            block_kind: "service".into(),
            line_range: (10, 14),
            reason: LossReason::ExtensionNotEnabled,
            raw_excerpt: "service Foo {}".into(),
        });
        let md = r.render_markdown();
        assert!(md.contains("foo.idl"));
        assert!(md.contains("50.0%"));
        assert!(md.contains("extension_not_enabled"));
        assert!(md.contains("service Foo"));
    }
}
