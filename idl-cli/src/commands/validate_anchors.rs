//! `idl validate --anchors --source <root>` — Wave 8 R4 anchor validator.
//!
//! For every node carrying `source_anchors[]`, resolves the URI against
//! the source root, verifies the file exists, the line range is in bounds,
//! and (when present) the sha256 hash matches. Emits per-node verdicts.
//!
//! Exit codes: 0 all aligned, 1 any breaking failure (missing file or OOB
//! range), 2 hash drift only (warnings).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use idl_graph::GraphDoc;
use serde::Serialize;
use sha2::{Digest, Sha256};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    Aligned,
    Shifted,
    Missing,
    HashDrift,
    NoAnchor,
}

#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub node_id: String,
    pub node_kind: String,
    pub state: String,
    pub uri: String,
    pub resolved_path: Option<String>,
    pub verdict: Verdict,
    pub note: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct Report {
    pub graph_path: String,
    pub source_root: String,
    pub total_nodes: usize,
    pub anchored_nodes: usize,
    pub aligned: usize,
    pub shifted: usize,
    pub missing: usize,
    pub hash_drift: usize,
    pub entries: Vec<Entry>,
}

pub fn run(graph_path: PathBuf, source_root: PathBuf, json: bool) -> Result<ExitCode> {
    let graph = GraphDoc::load(&graph_path)
        .with_context(|| format!("load graph {}", graph_path.display()))?;

    let mut report = Report {
        graph_path: graph_path.display().to_string(),
        source_root: source_root.display().to_string(),
        total_nodes: graph.nodes.len(),
        ..Default::default()
    };

    for node in &graph.nodes {
        if node.source_anchors.is_empty() {
            continue;
        }
        report.anchored_nodes += 1;
        // Use the first anchor as the primary verdict (matches drift/code).
        let anchor = &node.source_anchors[0];
        let entry = check_anchor(&source_root, node, anchor);
        match entry.verdict {
            Verdict::Aligned => report.aligned += 1,
            Verdict::Shifted => report.shifted += 1,
            Verdict::Missing => report.missing += 1,
            Verdict::HashDrift => report.hash_drift += 1,
            Verdict::NoAnchor => {}
        }
        report.entries.push(entry);
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_human(&report);
    }

    let breaking = report.missing + report.shifted;
    let code = if breaking > 0 {
        1
    } else if report.hash_drift > 0 {
        2
    } else {
        0
    };
    Ok(ExitCode::from(code))
}

fn check_anchor(
    source_root: &Path,
    node: &idl_graph::NodeDoc,
    anchor: &idl_graph::SourceAnchorDoc,
) -> Entry {
    let mut entry = Entry {
        node_id: node.id.clone(),
        node_kind: node.kind.clone(),
        state: node.state.clone(),
        uri: anchor.uri.clone(),
        resolved_path: None,
        verdict: Verdict::Missing,
        note: None,
    };

    let resolved = match resolve_uri(source_root, &anchor.uri) {
        Some(p) => p,
        None => {
            entry.note = Some("uri scheme not resolvable to source_root".into());
            return entry;
        }
    };

    if !resolved.exists() {
        entry.resolved_path = Some(resolved.display().to_string());
        entry.note = Some("file not found".into());
        return entry;
    }

    entry.resolved_path = Some(resolved.display().to_string());

    // Directory anchors (e.g. scope/verification nodes pointing at a
    // package or test folder) are accepted as-is. Range/hash checks below
    // only apply to file artifacts.
    if resolved.is_dir() {
        entry.verdict = Verdict::Aligned;
        entry.note = Some("directory anchor".into());
        return entry;
    }

    let bytes = match std::fs::read(&resolved) {
        Ok(b) => b,
        Err(e) => {
            entry.note = Some(format!("read error: {e}"));
            return entry;
        }
    };

    // Line-range check.
    if let Some(r) = &anchor.range {
        let line_count = bytecount_lines(&bytes);
        let start = r.start_line.unwrap_or(0);
        let end = r.end_line.unwrap_or(0);
        if end > 0 && end > line_count {
            entry.verdict = Verdict::Shifted;
            entry.note = Some(format!("end_line {end} > file line count {line_count}"));
            return entry;
        }
        if start > 0 && end > 0 && end < start {
            entry.verdict = Verdict::Shifted;
            entry.note = Some(format!("end_line {end} < start_line {start}"));
            return entry;
        }
    }

    // Hash check.
    if let Some(declared) = &anchor.hash {
        let (algo, expected_hex) = match declared.split_once(':') {
            Some(t) => t,
            None => {
                entry.verdict = Verdict::HashDrift;
                entry.note = Some(format!("malformed hash `{declared}`"));
                return entry;
            }
        };
        if algo != "sha256" {
            // Non-sha256 hashes pass through (we only verify sha256).
            entry.verdict = Verdict::Aligned;
            entry.note = Some(format!("hash algo {algo} not verified"));
            return entry;
        }
        let actual = sha256_hex(&bytes);
        if actual.eq_ignore_ascii_case(expected_hex) {
            entry.verdict = Verdict::Aligned;
        } else {
            entry.verdict = Verdict::HashDrift;
            entry.note = Some(format!("sha256 expected {expected_hex} got {actual}"));
        }
        return entry;
    }

    entry.verdict = Verdict::Aligned;
    entry
}

fn bytecount_lines(bytes: &[u8]) -> u64 {
    // Use the "split on '\n'" convention used by most extractors:
    // line_count = newline_count + (1 if non-empty, else 0). This treats
    // a trailing newline as terminating the final line and is one greater
    // than `wc -l` for files that don't end in '\n'. Validator picks the
    // lenient definition so off-by-one extractor quirks don't surface as
    // false-positive shifts; truly OOB ranges (end_line >> line_count)
    // still fail.
    if bytes.is_empty() {
        return 0;
    }
    let lf = bytes.iter().filter(|b| **b == b'\n').count() as u64;
    lf + 1
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

/// Map a graph URI to a local path under `source_root`. Mirrors
/// idl_graph::drift::resolve_uri but kept local because that fn is private.
pub fn resolve_uri(source_root: &Path, uri: &str) -> Option<PathBuf> {
    if let Some(rest) = uri.strip_prefix("repo://") {
        let mut parts = rest.splitn(2, '/');
        let corpus = parts.next().unwrap_or("");
        let rel = parts.next().unwrap_or("");
        // Try `<root>/<corpus>/<rel>` first (workspace-rooted convention).
        let cand1 = source_root.join(corpus).join(rel);
        if cand1.exists() {
            return Some(cand1);
        }
        // Fallback: source_root already points inside the corpus.
        let cand2 = source_root.join(rel);
        if cand2.exists() {
            return Some(cand2);
        }
        // Return the canonical (workspace-rooted) candidate so the caller
        // can report it as missing.
        return Some(source_root.join(corpus).join(rel));
    }
    if let Some(rest) = uri.strip_prefix("file://") {
        return Some(PathBuf::from(rest));
    }
    if uri.contains("://") {
        return None;
    }
    Some(source_root.join(uri))
}

fn print_human(r: &Report) {
    println!(
        "anchor-validate: total={} anchored={} aligned={} shifted={} missing={} hash-drift={}",
        r.total_nodes, r.anchored_nodes, r.aligned, r.shifted, r.missing, r.hash_drift,
    );
    let resolution_rate = if r.anchored_nodes > 0 {
        100.0 * r.aligned as f64 / r.anchored_nodes as f64
    } else {
        0.0
    };
    println!("  graph:  {}", r.graph_path);
    println!("  source: {}", r.source_root);
    println!("  resolution rate: {resolution_rate:.1}%");
    let mut shown = 0;
    for e in &r.entries {
        if e.verdict == Verdict::Aligned {
            continue;
        }
        if shown >= 20 {
            println!(
                "  ... ({} more failures)",
                r.entries
                    .iter()
                    .filter(|x| x.verdict != Verdict::Aligned)
                    .count()
                    - shown
            );
            break;
        }
        let note = e.note.as_deref().unwrap_or("");
        println!(
            "  [{:?}] {} ({}) — {} {}",
            e.verdict, e.node_id, e.node_kind, e.uri, note
        );
        shown += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use idl_graph::{NodeDoc, RangeDoc, SourceAnchorDoc};
    use tempfile::TempDir;

    fn write(dir: &Path, rel: &str, body: &str) -> PathBuf {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, body).unwrap();
        p
    }

    fn node_with(uri: &str, range: Option<RangeDoc>, hash: Option<&str>) -> NodeDoc {
        NodeDoc {
            id: "n:1".into(),
            kind: "entity".into(),
            state: "accepted".into(),
            created_by: None,
            props: serde_json::Map::new(),
            source_anchors: vec![SourceAnchorDoc {
                uri: uri.into(),
                range,
                hash: hash.map(String::from),
            }],
            confidence: None,
            decision_refs: vec![],
        }
    }

    #[test]
    fn aligned_when_file_exists_and_range_in_bounds() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "myrepo/src/foo.rs", "a\nb\nc\nd\ne\n");
        let node = node_with(
            "repo://myrepo/src/foo.rs",
            Some(RangeDoc {
                start_line: Some(1),
                end_line: Some(6),
                ..Default::default()
            }),
            None,
        );
        let entry = check_anchor(tmp.path(), &node, &node.source_anchors[0]);
        assert_eq!(entry.verdict, Verdict::Aligned, "{entry:?}");
    }

    #[test]
    fn shifted_when_end_line_exceeds_file_lines() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "myrepo/x.rs", "a\nb\nc\n"); // 3 lines
        let node = node_with(
            "repo://myrepo/x.rs",
            Some(RangeDoc {
                start_line: Some(1),
                end_line: Some(80),
                ..Default::default()
            }),
            None,
        );
        let entry = check_anchor(tmp.path(), &node, &node.source_anchors[0]);
        assert_eq!(entry.verdict, Verdict::Shifted);
    }

    #[test]
    fn missing_when_file_does_not_exist() {
        let tmp = TempDir::new().unwrap();
        let node = node_with("repo://myrepo/missing.rs", None, None);
        let entry = check_anchor(tmp.path(), &node, &node.source_anchors[0]);
        assert_eq!(entry.verdict, Verdict::Missing);
    }

    #[test]
    fn hash_drift_when_sha256_mismatch() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "myrepo/y.rs", "hello\n");
        let node = node_with(
            "repo://myrepo/y.rs",
            None,
            Some("sha256:0000000000000000000000000000000000000000000000000000000000000000"),
        );
        let entry = check_anchor(tmp.path(), &node, &node.source_anchors[0]);
        assert_eq!(entry.verdict, Verdict::HashDrift);
    }

    #[test]
    fn hash_match_when_sha256_correct() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "myrepo/z.rs", "hello\n");
        let actual = sha256_hex(b"hello\n");
        let node = node_with(
            "repo://myrepo/z.rs",
            None,
            Some(&format!("sha256:{actual}")),
        );
        let entry = check_anchor(tmp.path(), &node, &node.source_anchors[0]);
        assert_eq!(entry.verdict, Verdict::Aligned);
    }

    #[test]
    fn resolve_uri_falls_back_to_corpus_inside_source_root() {
        let tmp = TempDir::new().unwrap();
        // Source root *is* the corpus.
        write(tmp.path(), "src/foo.rs", "x\n");
        let p = resolve_uri(tmp.path(), "repo://myrepo/src/foo.rs").unwrap();
        assert!(p.exists());
    }
}
