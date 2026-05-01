//! Graph-level drift detection (P1.7).
//!
//! Compares two [`GraphDoc`]s (a baseline and a candidate) and produces a
//! [`DriftReport`] that classifies every change as `breaking`, `additive`,
//! or `cosmetic`. The detector is intentionally schema-aware: kernel
//! constructs leaving the `accepted` state, or accepted nodes disappearing,
//! count as breaking; new constructs in the candidate count as additive;
//! source-anchor range moves with identical content count as cosmetic.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::doc::{EdgeDoc, GraphDoc, NodeDoc};

/// Severity for a single drift event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DriftSeverity {
    Breaking,
    Additive,
    Cosmetic,
}

/// Kind of drift event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DriftEvent {
    NodeAdded { id: String, node_kind: String, state: String },
    NodeRemoved { id: String, node_kind: String, state: String },
    NodeStateChanged { id: String, from: String, to: String },
    NodePropsChanged {
        id: String,
        node_kind: String,
        changed: Vec<PropChange>,
    },
    NodeAnchorMoved { id: String, before: String, after: String },
    EdgeAdded { id: String, edge_kind: String, from: String, to: String },
    EdgeRemoved { id: String, edge_kind: String, from: String, to: String },
    EdgeRetargeted { id: String, before: String, after: String },
}

/// A single property change on a node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropChange {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<serde_json::Value>,
}

/// A single classified drift entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftEntry {
    pub severity: DriftSeverity,
    #[serde(flatten)]
    pub event: DriftEvent,
}

/// Aggregate drift report.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DriftReport {
    pub baseline_path: String,
    pub current_path: String,
    pub entries: Vec<DriftEntry>,
}

impl DriftReport {
    pub fn breaking(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.severity == DriftSeverity::Breaking)
            .count()
    }
    pub fn additive(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.severity == DriftSeverity::Additive)
            .count()
    }
    pub fn cosmetic(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.severity == DriftSeverity::Cosmetic)
            .count()
    }
    pub fn is_clean(&self) -> bool {
        self.entries.is_empty()
    }

    /// Process exit code per Wave 8 R3 spec:
    /// 0 no drift · 1 breaking · 2 additive only · 3 cosmetic only.
    pub fn exit_code(&self) -> u8 {
        if self.breaking() > 0 {
            1
        } else if self.additive() > 0 {
            2
        } else if self.cosmetic() > 0 {
            3
        } else {
            0
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }

    pub fn to_markdown(&self) -> String {
        let mut s = String::new();
        s.push_str("# IDL drift report\n\n");
        s.push_str(&format!("- baseline: `{}`\n", self.baseline_path));
        s.push_str(&format!("- current:  `{}`\n", self.current_path));
        s.push_str(&format!(
            "- breaking: **{}** · additive: **{}** · cosmetic: **{}**\n\n",
            self.breaking(),
            self.additive(),
            self.cosmetic()
        ));
        if self.entries.is_empty() {
            s.push_str("_No drift detected._\n");
            return s;
        }
        s.push_str("| severity | event |\n|---|---|\n");
        for e in &self.entries {
            let line = format!("| `{:?}` | `{}` |\n", e.severity, summarize_event(&e.event));
            s.push_str(&line);
        }
        s
    }

    pub fn to_human(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "drift: {} breaking, {} additive, {} cosmetic ({} total)\n",
            self.breaking(),
            self.additive(),
            self.cosmetic(),
            self.entries.len()
        ));
        s.push_str(&format!("  baseline: {}\n", self.baseline_path));
        s.push_str(&format!("  current:  {}\n", self.current_path));
        for e in &self.entries {
            s.push_str(&format!(
                "  [{:?}] {}\n",
                e.severity,
                summarize_event(&e.event)
            ));
        }
        s
    }
}

fn summarize_event(e: &DriftEvent) -> String {
    match e {
        DriftEvent::NodeAdded { id, node_kind, state } => {
            format!("+ node {id} ({node_kind}, {state})")
        }
        DriftEvent::NodeRemoved { id, node_kind, state } => {
            format!("- node {id} ({node_kind}, {state})")
        }
        DriftEvent::NodeStateChanged { id, from, to } => {
            format!("~ node {id} state {from} → {to}")
        }
        DriftEvent::NodePropsChanged { id, node_kind: _, changed } => {
            format!("~ node {id} props ({} changed)", changed.len())
        }
        DriftEvent::NodeAnchorMoved { id, before, after } => {
            format!("~ node {id} anchor moved {before} → {after}")
        }
        DriftEvent::EdgeAdded { id, edge_kind, from, to } => {
            format!("+ edge {id} ({edge_kind}: {from} → {to})")
        }
        DriftEvent::EdgeRemoved { id, edge_kind, from, to } => {
            format!("- edge {id} ({edge_kind}: {from} → {to})")
        }
        DriftEvent::EdgeRetargeted { id, before, after } => {
            format!("~ edge {id} retargeted {before} → {after}")
        }
    }
}

/// Compare two graph documents and produce a [`DriftReport`].
pub fn diff_graphs(
    baseline_path: impl Into<String>,
    baseline: &GraphDoc,
    current_path: impl Into<String>,
    current: &GraphDoc,
) -> DriftReport {
    let mut report = DriftReport {
        baseline_path: baseline_path.into(),
        current_path: current_path.into(),
        entries: Vec::new(),
    };

    let base_nodes: BTreeMap<&str, &NodeDoc> =
        baseline.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let cur_nodes: BTreeMap<&str, &NodeDoc> =
        current.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    let base_ids: BTreeSet<&str> = base_nodes.keys().copied().collect();
    let cur_ids: BTreeSet<&str> = cur_nodes.keys().copied().collect();

    // Removed.
    for id in base_ids.difference(&cur_ids) {
        let n = base_nodes[*id];
        let severity = if n.state == "accepted" {
            DriftSeverity::Breaking
        } else {
            DriftSeverity::Additive
        };
        report.entries.push(DriftEntry {
            severity,
            event: DriftEvent::NodeRemoved {
                id: n.id.clone(),
                node_kind: n.kind.clone(),
                state: n.state.clone(),
            },
        });
    }
    // Added.
    for id in cur_ids.difference(&base_ids) {
        let n = cur_nodes[*id];
        report.entries.push(DriftEntry {
            severity: DriftSeverity::Additive,
            event: DriftEvent::NodeAdded {
                id: n.id.clone(),
                node_kind: n.kind.clone(),
                state: n.state.clone(),
            },
        });
    }
    // Modified.
    for id in base_ids.intersection(&cur_ids) {
        let a = base_nodes[*id];
        let b = cur_nodes[*id];
        if a.state != b.state {
            let sev = match (a.state.as_str(), b.state.as_str()) {
                ("accepted", "drifted") | ("accepted", "rejected") => DriftSeverity::Breaking,
                _ => DriftSeverity::Additive,
            };
            report.entries.push(DriftEntry {
                severity: sev,
                event: DriftEvent::NodeStateChanged {
                    id: a.id.clone(),
                    from: a.state.clone(),
                    to: b.state.clone(),
                },
            });
        }
        let changed = diff_props(&a.props, &b.props);
        if !changed.is_empty() {
            let sev = if a.state == "accepted" {
                DriftSeverity::Breaking
            } else {
                DriftSeverity::Additive
            };
            report.entries.push(DriftEntry {
                severity: sev,
                event: DriftEvent::NodePropsChanged {
                    id: a.id.clone(),
                    node_kind: a.kind.clone(),
                    changed,
                },
            });
        }
        // Anchor moved cosmetically: same uri+hash but range differs.
        if let (Some(ba), Some(ca)) = (a.source_anchors.first(), b.source_anchors.first()) {
            if ba.uri == ca.uri && ba.hash == ca.hash && ba.range != ca.range {
                report.entries.push(DriftEntry {
                    severity: DriftSeverity::Cosmetic,
                    event: DriftEvent::NodeAnchorMoved {
                        id: a.id.clone(),
                        before: format_range(&ba.range),
                        after: format_range(&ca.range),
                    },
                });
            }
        }
    }

    // Edges.
    let base_edges: BTreeMap<&str, &EdgeDoc> =
        baseline.edges.iter().map(|e| (e.id.as_str(), e)).collect();
    let cur_edges: BTreeMap<&str, &EdgeDoc> =
        current.edges.iter().map(|e| (e.id.as_str(), e)).collect();
    let be: BTreeSet<&str> = base_edges.keys().copied().collect();
    let ce: BTreeSet<&str> = cur_edges.keys().copied().collect();
    for id in be.difference(&ce) {
        let e = base_edges[*id];
        report.entries.push(DriftEntry {
            severity: DriftSeverity::Breaking,
            event: DriftEvent::EdgeRemoved {
                id: e.id.clone(),
                edge_kind: e.kind.clone(),
                from: e.from.clone(),
                to: e.to.clone(),
            },
        });
    }
    for id in ce.difference(&be) {
        let e = cur_edges[*id];
        report.entries.push(DriftEntry {
            severity: DriftSeverity::Additive,
            event: DriftEvent::EdgeAdded {
                id: e.id.clone(),
                edge_kind: e.kind.clone(),
                from: e.from.clone(),
                to: e.to.clone(),
            },
        });
    }
    for id in be.intersection(&ce) {
        let a = base_edges[*id];
        let b = cur_edges[*id];
        if a.from != b.from || a.to != b.to || a.kind != b.kind {
            report.entries.push(DriftEntry {
                severity: DriftSeverity::Breaking,
                event: DriftEvent::EdgeRetargeted {
                    id: a.id.clone(),
                    before: format!("{} {}→{}", a.kind, a.from, a.to),
                    after: format!("{} {}→{}", b.kind, b.from, b.to),
                },
            });
        }
    }

    report
}

fn format_range(r: &Option<crate::doc::RangeDoc>) -> String {
    match r {
        None => "-".into(),
        Some(r) => format!(
            "{}:{}-{}",
            r.start_line.map(|x| x.to_string()).unwrap_or_else(|| "?".into()),
            r.start_byte.map(|x| x.to_string()).unwrap_or_else(|| "?".into()),
            r.end_line.map(|x| x.to_string()).unwrap_or_else(|| "?".into()),
        ),
    }
}

fn diff_props(
    a: &serde_json::Map<String, serde_json::Value>,
    b: &serde_json::Map<String, serde_json::Value>,
) -> Vec<PropChange> {
    let mut out = Vec::new();
    let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    for k in keys {
        match (a.get(k), b.get(k)) {
            (Some(va), Some(vb)) if va != vb => out.push(PropChange {
                path: k.clone(),
                before: Some(va.clone()),
                after: Some(vb.clone()),
            }),
            (Some(va), None) => out.push(PropChange {
                path: k.clone(),
                before: Some(va.clone()),
                after: None,
            }),
            (None, Some(vb)) => out.push(PropChange {
                path: k.clone(),
                before: None,
                after: Some(vb.clone()),
            }),
            _ => {}
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Code-anchor drift (Part A.2): re-anchor source_anchors against a code root.
// ---------------------------------------------------------------------------

/// Per-node verdict from `idl drift code`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnchorVerdict {
    Aligned,
    Shifted,
    Missing,
    NewInCode,
}

/// Per-node anchor verdict.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnchorEntry {
    pub node_id: String,
    pub node_kind: String,
    pub verdict: AnchorVerdict,
    pub uri: String,
    pub resolved_path: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AnchorReport {
    pub graph_path: String,
    pub source_root: String,
    pub entries: Vec<AnchorEntry>,
}

impl AnchorReport {
    pub fn aligned(&self) -> usize {
        self.entries.iter().filter(|e| e.verdict == AnchorVerdict::Aligned).count()
    }
    pub fn shifted(&self) -> usize {
        self.entries.iter().filter(|e| e.verdict == AnchorVerdict::Shifted).count()
    }
    pub fn missing(&self) -> usize {
        self.entries.iter().filter(|e| e.verdict == AnchorVerdict::Missing).count()
    }
    pub fn new_in_code(&self) -> usize {
        self.entries.iter().filter(|e| e.verdict == AnchorVerdict::NewInCode).count()
    }
    /// 0 aligned · 1 missing (breaking) · 2 new-in-code (additive) · 3 shifted (cosmetic).
    pub fn exit_code(&self) -> u8 {
        if self.missing() > 0 {
            1
        } else if self.new_in_code() > 0 {
            2
        } else if self.shifted() > 0 {
            3
        } else {
            0
        }
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }
    pub fn to_markdown(&self) -> String {
        let mut s = String::new();
        s.push_str("# IDL code-drift report\n\n");
        s.push_str(&format!("- graph:  `{}`\n", self.graph_path));
        s.push_str(&format!("- source: `{}`\n\n", self.source_root));
        s.push_str(&format!(
            "aligned **{}** · shifted **{}** · missing **{}** · new-in-code **{}**\n\n",
            self.aligned(),
            self.shifted(),
            self.missing(),
            self.new_in_code()
        ));
        s.push_str("| node | kind | verdict | uri |\n|---|---|---|---|\n");
        for e in &self.entries {
            s.push_str(&format!(
                "| `{}` | `{}` | `{:?}` | `{}` |\n",
                e.node_id, e.node_kind, e.verdict, e.uri
            ));
        }
        s
    }
    pub fn to_human(&self) -> String {
        let mut s = format!(
            "code-drift: aligned={} shifted={} missing={} new-in-code={}\n",
            self.aligned(),
            self.shifted(),
            self.missing(),
            self.new_in_code()
        );
        for e in &self.entries {
            s.push_str(&format!(
                "  [{:?}] {} ({})\n",
                e.verdict, e.node_id, e.node_kind
            ));
        }
        s
    }
}

/// Compute code-anchor drift: walk every node with `source_anchors`, try to
/// resolve each anchor against `source_root`, and classify.
pub fn diff_against_source(
    graph_path: impl Into<String>,
    graph: &GraphDoc,
    source_root: &std::path::Path,
) -> AnchorReport {
    let mut report = AnchorReport {
        graph_path: graph_path.into(),
        source_root: source_root.display().to_string(),
        ..Default::default()
    };
    for node in &graph.nodes {
        if node.source_anchors.is_empty() {
            continue;
        }
        let anchor = &node.source_anchors[0];
        let local = resolve_uri(source_root, &anchor.uri);
        let (verdict, resolved_path, note) = match local {
            Some(p) if p.exists() => {
                if let Some(r) = &anchor.range {
                    let lines = std::fs::read_to_string(&p)
                        .map(|t| t.lines().count() as u64)
                        .unwrap_or(0);
                    let end = r.end_line.unwrap_or(0);
                    if end > 0 && end > lines {
                        (
                            AnchorVerdict::Shifted,
                            Some(p.display().to_string()),
                            Some(format!(
                                "anchor end_line {} exceeds file length {}",
                                end, lines
                            )),
                        )
                    } else {
                        (AnchorVerdict::Aligned, Some(p.display().to_string()), None)
                    }
                } else {
                    (AnchorVerdict::Aligned, Some(p.display().to_string()), None)
                }
            }
            Some(p) => (
                AnchorVerdict::Missing,
                Some(p.display().to_string()),
                Some("file not found".into()),
            ),
            None => (
                AnchorVerdict::Missing,
                None,
                Some("uri scheme not resolvable to source_root".into()),
            ),
        };
        report.entries.push(AnchorEntry {
            node_id: node.id.clone(),
            node_kind: node.kind.clone(),
            verdict,
            uri: anchor.uri.clone(),
            resolved_path,
            note,
        });
    }
    report
}

/// Map a graph URI to a local path under `source_root`. Supports
/// `repo://<corpus>/<rel>`, `file://<abs>`, and bare relative paths.
fn resolve_uri(source_root: &std::path::Path, uri: &str) -> Option<std::path::PathBuf> {
    if let Some(rest) = uri.strip_prefix("repo://") {
        // repo://<corpus>/<rel>... — try `<root>/<rel>` first, then `<root>/<corpus>/<rel>`.
        let mut parts = rest.splitn(2, '/');
        let corpus = parts.next().unwrap_or("");
        let rel = parts.next().unwrap_or("");
        let cand1 = source_root.join(rel);
        if cand1.exists() {
            return Some(cand1);
        }
        let cand2 = source_root.join(corpus).join(rel);
        return Some(cand2);
    }
    if let Some(rest) = uri.strip_prefix("file://") {
        return Some(std::path::PathBuf::from(rest));
    }
    if uri.contains("://") {
        return None;
    }
    Some(source_root.join(uri))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc_with(nodes: Vec<NodeDoc>, edges: Vec<EdgeDoc>) -> GraphDoc {
        GraphDoc {
            version: "0.1.0".into(),
            metadata: serde_json::Value::Null,
            nodes,
            edges,
            extensions: None,
        }
    }
    fn n(id: &str, kind: &str, state: &str) -> NodeDoc {
        NodeDoc {
            id: id.into(),
            kind: kind.into(),
            state: state.into(),
            created_by: None,
            props: serde_json::Map::new(),
            source_anchors: vec![],
            confidence: None,
            decision_refs: vec![],
        }
    }
    fn e(id: &str, kind: &str, from: &str, to: &str) -> EdgeDoc {
        EdgeDoc {
            id: id.into(),
            kind: kind.into(),
            from: from.into(),
            to: to.into(),
            props: serde_json::Map::new(),
        }
    }

    #[test]
    fn detects_added_and_removed_nodes() {
        let a = doc_with(vec![n("e:1", "entity", "accepted")], vec![]);
        let b = doc_with(vec![n("e:2", "entity", "proposed")], vec![]);
        let r = diff_graphs("a", &a, "b", &b);
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.breaking(), 1, "removing accepted is breaking: {:?}", r.entries);
        assert_eq!(r.additive(), 1);
    }

    #[test]
    fn detects_state_change_and_props_change() {
        let mut n1 = n("op:x", "operation", "accepted");
        n1.props.insert("name".into(), serde_json::json!("foo"));
        let mut n2 = n("op:x", "operation", "drifted");
        n2.props.insert("name".into(), serde_json::json!("bar"));
        let a = doc_with(vec![n1], vec![]);
        let b = doc_with(vec![n2], vec![]);
        let r = diff_graphs("a", &a, "b", &b);
        assert!(r.entries.iter().any(|e| matches!(e.event, DriftEvent::NodeStateChanged{..})));
        assert!(r.entries.iter().any(|e| matches!(e.event, DriftEvent::NodePropsChanged{..})));
        assert!(r.breaking() > 0);
    }

    #[test]
    fn detects_edge_changes() {
        let a = doc_with(
            vec![n("a", "intent", "accepted"), n("b", "api", "accepted")],
            vec![e("eid", "realizes", "b", "a")],
        );
        let b = doc_with(
            vec![n("a", "intent", "accepted"), n("b", "api", "accepted")],
            vec![e("eid2", "realizes", "b", "a")],
        );
        let r = diff_graphs("x", &a, "y", &b);
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.breaking(), 1); // removed edge is breaking
        assert_eq!(r.additive(), 1);
    }

    #[test]
    fn no_drift_means_clean() {
        let a = doc_with(vec![n("e:1", "entity", "accepted")], vec![]);
        let r = diff_graphs("a", &a, "a", &a);
        assert!(r.is_clean());
        assert_eq!(r.exit_code(), 0);
    }
}
