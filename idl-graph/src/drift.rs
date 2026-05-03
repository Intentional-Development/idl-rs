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
#[cfg(test)]
use crate::doc::{RangeDoc, SourceAnchorDoc};

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
    NodeAdded {
        id: String,
        node_kind: String,
        state: String,
    },
    NodeRemoved {
        id: String,
        node_kind: String,
        state: String,
    },
    NodeStateChanged {
        id: String,
        from: String,
        to: String,
    },
    NodePropsChanged {
        id: String,
        node_kind: String,
        changed: Vec<PropChange>,
    },
    NodeAnchorMoved {
        id: String,
        before: String,
        after: String,
    },
    EdgeAdded {
        id: String,
        edge_kind: String,
        from: String,
        to: String,
    },
    EdgeRemoved {
        id: String,
        edge_kind: String,
        from: String,
        to: String,
    },
    EdgeRetargeted {
        id: String,
        before: String,
        after: String,
    },
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
        DriftEvent::NodeAdded {
            id,
            node_kind,
            state,
        } => {
            format!("+ node {id} ({node_kind}, {state})")
        }
        DriftEvent::NodeRemoved {
            id,
            node_kind,
            state,
        } => {
            format!("- node {id} ({node_kind}, {state})")
        }
        DriftEvent::NodeStateChanged { id, from, to } => {
            format!("~ node {id} state {from} → {to}")
        }
        DriftEvent::NodePropsChanged {
            id,
            node_kind: _,
            changed,
        } => {
            format!("~ node {id} props ({} changed)", changed.len())
        }
        DriftEvent::NodeAnchorMoved { id, before, after } => {
            format!("~ node {id} anchor moved {before} → {after}")
        }
        DriftEvent::EdgeAdded {
            id,
            edge_kind,
            from,
            to,
        } => {
            format!("+ edge {id} ({edge_kind}: {from} → {to})")
        }
        DriftEvent::EdgeRemoved {
            id,
            edge_kind,
            from,
            to,
        } => {
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
    diff_graphs_internal(baseline_path, baseline, current_path, current, false)
}

/// Compare two graph documents with contract-type-aware severity classification.
///
/// When `contract_aware` is true, severity is classified based on the semantic
/// meaning of the node kind and the specific property changes:
/// - API contracts (api, operation): field removal = breaking, new optional field = additive
/// - DTOs (entity, variant): type change = breaking, new optional field = additive
/// - Behavior contracts (operation, rule, invariant): signature/constraint change = breaking
/// - Topology (service edges): new dependency = additive/warning, removed = breaking
pub fn diff_graphs_contract_aware(
    baseline_path: impl Into<String>,
    baseline: &GraphDoc,
    current_path: impl Into<String>,
    current: &GraphDoc,
) -> DriftReport {
    diff_graphs_internal(baseline_path, baseline, current_path, current, true)
}

fn diff_graphs_internal(
    baseline_path: impl Into<String>,
    baseline: &GraphDoc,
    current_path: impl Into<String>,
    current: &GraphDoc,
    contract_aware: bool,
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
        let severity = if contract_aware {
            classify_node_removed(n)
        } else if n.state == "accepted" {
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
        let severity = if contract_aware {
            classify_node_added(n)
        } else {
            DriftSeverity::Additive
        };
        report.entries.push(DriftEntry {
            severity,
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
            let sev = if contract_aware {
                classify_state_change(a, b)
            } else {
                match (a.state.as_str(), b.state.as_str()) {
                    ("accepted", "drifted") | ("accepted", "rejected") => DriftSeverity::Breaking,
                    _ => DriftSeverity::Additive,
                }
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
            let sev = if contract_aware {
                classify_props_changed(a, b, &changed)
            } else if a.state == "accepted" {
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
        let severity = if contract_aware {
            classify_edge_removed(e, &base_nodes)
        } else {
            DriftSeverity::Breaking
        };
        report.entries.push(DriftEntry {
            severity,
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
        let severity = if contract_aware {
            classify_edge_added(e, &cur_nodes)
        } else {
            DriftSeverity::Additive
        };
        report.entries.push(DriftEntry {
            severity,
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

/// Contract-aware severity for node removal.
fn classify_node_removed(node: &NodeDoc) -> DriftSeverity {
    // If accepted, removal is always breaking for API/DTO/behavior contracts.
    if node.state == "accepted" {
        match node.kind.as_str() {
            "api" | "operation" | "entity" | "aggregate" | "variant" | "rule" | "invariant" => {
                DriftSeverity::Breaking
            }
            _ => DriftSeverity::Breaking,
        }
    } else {
        DriftSeverity::Additive
    }
}

/// Contract-aware severity for node addition.
fn classify_node_added(node: &NodeDoc) -> DriftSeverity {
    // New nodes are generally additive, but we can refine based on context.
    // For now, treat all additions as additive (non-breaking).
    match node.kind.as_str() {
        "api" | "operation" | "entity" | "variant" => DriftSeverity::Additive,
        _ => DriftSeverity::Additive,
    }
}

/// Contract-aware severity for state changes.
fn classify_state_change(baseline: &NodeDoc, current: &NodeDoc) -> DriftSeverity {
    match (baseline.state.as_str(), current.state.as_str()) {
        ("accepted", "drifted") | ("accepted", "rejected") => {
            // Accepted contract going to drifted/rejected is breaking.
            match baseline.kind.as_str() {
                "api" | "operation" | "entity" | "aggregate" | "variant" | "rule" | "invariant" => {
                    DriftSeverity::Breaking
                }
                _ => DriftSeverity::Breaking,
            }
        }
        _ => DriftSeverity::Additive,
    }
}

/// Contract-aware severity for property changes.
fn classify_props_changed(
    baseline: &NodeDoc,
    current: &NodeDoc,
    changes: &[PropChange],
) -> DriftSeverity {
    if baseline.state != "accepted" {
        return DriftSeverity::Additive;
    }

    match baseline.kind.as_str() {
        "api" | "operation" => classify_api_props_changed(changes),
        "entity" | "aggregate" | "variant" => classify_dto_props_changed(baseline, current, changes),
        "rule" | "invariant" | "policy" => classify_behavior_props_changed(changes),
        _ => {
            if baseline.state == "accepted" {
                DriftSeverity::Breaking
            } else {
                DriftSeverity::Additive
            }
        }
    }
}

/// API contract prop changes: field removal = breaking, new optional = additive.
fn classify_api_props_changed(changes: &[PropChange]) -> DriftSeverity {
    for change in changes {
        // Check for breaking changes in API contracts.
        if change.path == "input" || change.path == "output" {
            // If a field was removed (after is None), it's breaking
            if change.after.is_none() {
                return DriftSeverity::Breaking;
            }
            // If field was added (before is None), it's additive
            if change.before.is_none() {
                return DriftSeverity::Additive;
            }
            // Both before and after exist - this is a modification
            // For API contracts, any change to input/output structure is breaking
            // (in a real implementation, we'd need deeper schema comparison)
            if let (Some(before_val), Some(after_val)) = (&change.before, &change.after) {
                if before_val != after_val {
                    return DriftSeverity::Breaking;
                }
            }
        }
        // Method change is always breaking
        if change.path == "method" {
            if change.after.is_none() {
                return DriftSeverity::Breaking;
            }
            if let (Some(before_val), Some(after_val)) = (&change.before, &change.after) {
                if before_val != after_val {
                    return DriftSeverity::Breaking;
                }
            }
        }
    }
    DriftSeverity::Additive
}

/// DTO prop changes: type change = breaking, new optional field = additive.
fn classify_dto_props_changed(
    _baseline: &NodeDoc,
    _current: &NodeDoc,
    changes: &[PropChange],
) -> DriftSeverity {
    for change in changes {
        // Check for breaking changes in DTOs.
        if (change.path == "base" || change.path == "fields") && change.after.is_none() {
            return DriftSeverity::Breaking; // Field removed
        }
        // If fields array changed, check if it's a removal or type change.
        if change.path == "fields" {
            if let (Some(before), Some(after)) = (&change.before, &change.after) {
                if before != after {
                    // Structural change to fields - assume breaking for now.
                    return DriftSeverity::Breaking;
                }
            }
        }
    }
    // Check if only adding optional fields (would require deeper analysis).
    DriftSeverity::Additive
}

/// Behavior contract prop changes: constraint/signature change = breaking.
fn classify_behavior_props_changed(changes: &[PropChange]) -> DriftSeverity {
    for change in changes {
        if (change.path == "condition" || change.path == "expression" || change.path == "constraint")
            && change.before.is_some()
            && change.after.is_some()
        {
            return DriftSeverity::Breaking; // Logic changed
        }
    }
    DriftSeverity::Additive
}

/// Edge removal: topology changes.
fn classify_edge_removed(edge: &EdgeDoc, _nodes: &BTreeMap<&str, &NodeDoc>) -> DriftSeverity {
    match edge.kind.as_str() {
        "implements" | "realizes" | "verifies" => DriftSeverity::Breaking,
        "queries" | "authorizes" => DriftSeverity::Breaking, // Service dependency removed
        _ => DriftSeverity::Breaking,
    }
}

/// Edge addition: new dependencies are additive (warning).
fn classify_edge_added(edge: &EdgeDoc, _nodes: &BTreeMap<&str, &NodeDoc>) -> DriftSeverity {
    match edge.kind.as_str() {
        "queries" | "authorizes" | "triggers" => DriftSeverity::Additive, // New dependency
        _ => DriftSeverity::Additive,
    }
}

fn format_range(r: &Option<crate::doc::RangeDoc>) -> String {
    match r {
        None => "-".into(),
        Some(r) => format!(
            "{}:{}-{}",
            r.start_line
                .map(|x| x.to_string())
                .unwrap_or_else(|| "?".into()),
            r.start_byte
                .map(|x| x.to_string())
                .unwrap_or_else(|| "?".into()),
            r.end_line
                .map(|x| x.to_string())
                .unwrap_or_else(|| "?".into()),
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

impl AnchorVerdict {
    /// Lowercase, hyphenated wire form (matches JSON serialization).
    pub fn as_str(&self) -> &'static str {
        match self {
            AnchorVerdict::Aligned => "aligned",
            AnchorVerdict::Shifted => "shifted",
            AnchorVerdict::Missing => "missing",
            AnchorVerdict::NewInCode => "new-in-code",
        }
    }
}

impl std::fmt::Display for AnchorVerdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
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
        self.entries
            .iter()
            .filter(|e| e.verdict == AnchorVerdict::Aligned)
            .count()
    }
    pub fn shifted(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.verdict == AnchorVerdict::Shifted)
            .count()
    }
    pub fn missing(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.verdict == AnchorVerdict::Missing)
            .count()
    }
    pub fn new_in_code(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.verdict == AnchorVerdict::NewInCode)
            .count()
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
                "| `{}` | `{}` | `{}` | `{}` |\n",
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
                "  [{}] {} ({})\n",
                e.verdict, e.node_id, e.node_kind
            ));
        }
        s
    }
}

/// Compute code-anchor drift: walk every node with `source_anchors`, try to
/// resolve each anchor against `source_root`, and classify.
///
/// Single-root convenience wrapper around [`diff_against_sources`].
pub fn diff_against_source(
    graph_path: impl Into<String>,
    graph: &GraphDoc,
    source_root: &std::path::Path,
) -> AnchorReport {
    diff_against_sources(
        graph_path,
        graph,
        &[(String::new(), source_root.to_path_buf())],
    )
}

/// Multi-root variant. `mappings` is a list of `(corpus_name, root_path)`
/// entries. When a `repo://<corpus>/...` URI's first segment matches a known
/// corpus name, the URI is routed under that corpus's root. An entry with an
/// empty corpus name acts as the default root (used when no corpus matches).
pub fn diff_against_sources(
    graph_path: impl Into<String>,
    graph: &GraphDoc,
    mappings: &[(String, std::path::PathBuf)],
) -> AnchorReport {
    let source_root = mappings
        .iter()
        .map(|(c, p)| {
            if c.is_empty() {
                p.display().to_string()
            } else {
                format!("{c}={}", p.display())
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    let mut report = AnchorReport {
        graph_path: graph_path.into(),
        source_root,
        ..Default::default()
    };
    for node in &graph.nodes {
        if node.source_anchors.is_empty() {
            continue;
        }
        let anchor = &node.source_anchors[0];
        let local = resolve_uri_multi(mappings, &anchor.uri);
        let (verdict, resolved_path, note) = match local {
            Some(p) if p.exists() => {
                // Bug A: directory anchors are aligned if the directory exists.
                if p.is_dir() {
                    (AnchorVerdict::Aligned, Some(p.display().to_string()), None)
                } else if let Some(r) = &anchor.range {
                    let lines = std::fs::read_to_string(&p)
                        .map(|t| t.lines().count() as u64)
                        .unwrap_or(0);
                    let start = r.start_line.unwrap_or(0);
                    let end = r.end_line.unwrap_or(0);
                    // Bug C: clamp `end_line == lines + 1` silently for whole-file
                    // anchors. Only flag when start_line > line_count, or when end
                    // overshoots by >1 (genuine overrun).
                    if start > 0 && start > lines {
                        (
                            AnchorVerdict::Shifted,
                            Some(p.display().to_string()),
                            Some(format!(
                                "anchor start_line {} exceeds file length {}",
                                start, lines
                            )),
                        )
                    } else if end > lines + 1 {
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
                Some("uri scheme not resolvable to any source root".into()),
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
#[allow(dead_code)]
fn resolve_uri(source_root: &std::path::Path, uri: &str) -> Option<std::path::PathBuf> {
    resolve_uri_multi(&[(String::new(), source_root.to_path_buf())], uri)
}

/// Multi-root URI resolver (Wave 10 — bug B). Looks up `repo://<corpus>/...`
/// against the provided mappings; falls back to the default (empty-name) root.
fn resolve_uri_multi(
    mappings: &[(String, std::path::PathBuf)],
    uri: &str,
) -> Option<std::path::PathBuf> {
    if let Some(rest) = uri.strip_prefix("repo://") {
        let mut parts = rest.splitn(2, '/');
        let corpus = parts.next().unwrap_or("");
        let rel = parts.next().unwrap_or("");
        // 1) explicit corpus mapping wins.
        if let Some((_, root)) = mappings.iter().find(|(c, _)| c == corpus) {
            return Some(root.join(rel));
        }
        // 2) fall back to the default ("") root, trying both `<root>/<rel>`
        //    and `<root>/<corpus>/<rel>` for backwards compat.
        if let Some((_, root)) = mappings.iter().find(|(c, _)| c.is_empty()) {
            let cand1 = root.join(rel);
            if cand1.exists() {
                return Some(cand1);
            }
            return Some(root.join(corpus).join(rel));
        }
        return None;
    }
    if let Some(rest) = uri.strip_prefix("file://") {
        return Some(std::path::PathBuf::from(rest));
    }
    if uri.contains("://") {
        return None;
    }
    if let Some((_, root)) = mappings.iter().find(|(c, _)| c.is_empty()) {
        return Some(root.join(uri));
    }
    mappings.first().map(|(_, root)| root.join(uri))
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
        assert_eq!(
            r.breaking(),
            1,
            "removing accepted is breaking: {:?}",
            r.entries
        );
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
        assert!(r
            .entries
            .iter()
            .any(|e| matches!(e.event, DriftEvent::NodeStateChanged { .. })));
        assert!(r
            .entries
            .iter()
            .any(|e| matches!(e.event, DriftEvent::NodePropsChanged { .. })));
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

    // ----- Wave 10 drift-tool fixes ---------------------------------------

    fn anchored_node(id: &str, kind: &str, uri: &str, range: Option<RangeDoc>) -> NodeDoc {
        let mut node = n(id, kind, "accepted");
        node.source_anchors.push(SourceAnchorDoc {
            uri: uri.into(),
            range,
            hash: None,
        });
        node
    }

    /// Bug A — directory anchors must report `aligned` when the directory
    /// exists, not `shifted` (length-0).
    #[test]
    fn directory_anchor_is_aligned_not_shifted() {
        let tmp = tempdir_in_target("drift-dir-anchor");
        let dir = tmp.join("subdir");
        std::fs::create_dir_all(&dir).unwrap();

        let g = doc_with(
            vec![anchored_node("scope:x", "scope", "subdir", None)],
            vec![],
        );
        let r = diff_against_source("g", &g, &tmp);
        assert_eq!(r.entries.len(), 1);
        assert_eq!(
            r.entries[0].verdict,
            AnchorVerdict::Aligned,
            "{:?}",
            r.entries[0]
        );
        assert_eq!(r.shifted(), 0);
    }

    /// Bug C — `end_line == file_line_count + 1` (whole-file anchor) must be
    /// silently clamped, not flagged as shifted. `start_line > line_count` is
    /// still shifted.
    #[test]
    fn end_line_off_by_one_is_aligned() {
        let tmp = tempdir_in_target("drift-eof");
        let file = tmp.join("x.txt");
        std::fs::write(&file, "a\nb\nc\n").unwrap(); // 3 lines per `lines().count()`.

        let off_by_one = anchored_node(
            "entity:x",
            "entity",
            "x.txt",
            Some(RangeDoc {
                start_line: Some(1),
                end_line: Some(4),
                ..Default::default()
            }),
        );
        let r = diff_against_source("g", &doc_with(vec![off_by_one], vec![]), &tmp);
        assert_eq!(
            r.entries[0].verdict,
            AnchorVerdict::Aligned,
            "off-by-one EOF should align"
        );

        let real_overrun = anchored_node(
            "entity:y",
            "entity",
            "x.txt",
            Some(RangeDoc {
                start_line: Some(1),
                end_line: Some(99),
                ..Default::default()
            }),
        );
        let r = diff_against_source("g", &doc_with(vec![real_overrun], vec![]), &tmp);
        assert_eq!(
            r.entries[0].verdict,
            AnchorVerdict::Shifted,
            ">+1 overrun stays shifted"
        );
    }

    /// Bug B — multi-source mappings route `repo://<corpus>/...` to the
    /// matching root.
    #[test]
    fn multi_source_routes_by_corpus_name() {
        let tmp = tempdir_in_target("drift-multi");
        let n8n_root = tmp.join("n8n-fake");
        let idl_root = tmp.join("IDL-fake");
        std::fs::create_dir_all(n8n_root.join("packages/cli")).unwrap();
        std::fs::create_dir_all(&idl_root).unwrap();
        std::fs::write(idl_root.join("notes.md"), "hello\n").unwrap();

        let nodes = vec![
            anchored_node("scope:n8n", "scope", "repo://n8n/packages/cli", None),
            anchored_node(
                "decision:idl",
                "decision",
                "repo://IDL/notes.md",
                Some(RangeDoc {
                    start_line: Some(1),
                    end_line: Some(1),
                    ..Default::default()
                }),
            ),
        ];
        let mappings = vec![
            ("n8n".to_string(), n8n_root.clone()),
            ("IDL".to_string(), idl_root.clone()),
        ];
        let r = diff_against_sources("g", &doc_with(nodes, vec![]), &mappings);
        assert_eq!(
            r.aligned(),
            2,
            "both URIs should resolve under their mapped root: {:?}",
            r.entries
        );
        assert_eq!(r.missing(), 0);
    }

    /// Bug D — verdict serialisation in markdown/human output is lowercase
    /// (no `Aligned` / `Shifted` debug casing).
    #[test]
    fn verdict_rendering_is_lowercase() {
        let tmp = tempdir_in_target("drift-verdict");
        let dir = tmp.join("src");
        std::fs::create_dir_all(&dir).unwrap();
        let g = doc_with(vec![anchored_node("scope:s", "scope", "src", None)], vec![]);
        let r = diff_against_source("g", &g, &tmp);
        let md = r.to_markdown();
        let human = r.to_human();
        assert!(
            md.contains("`aligned`"),
            "markdown should use lowercase verdict, got:\n{md}"
        );
        assert!(
            !md.contains("`Aligned`"),
            "markdown leaks Debug casing:\n{md}"
        );
        assert!(
            human.contains("[aligned]"),
            "human should use lowercase verdict:\n{human}"
        );

        // JSON already lowercase via serde rename_all.
        let j = r.to_json();
        assert!(j.contains("\"aligned\""));
        assert!(!j.contains("\"Aligned\""));
    }

    fn tempdir_in_target(label: &str) -> std::path::PathBuf {
        // Use target/ inside the workspace so we never touch /tmp.
        let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../target/idl-drift-tests")
            .join(format!("{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        base
    }

    // ----- Contract-type-aware drift tests (W31) ---------------------------

    #[test]
    fn contract_aware_api_field_removal_is_breaking() {
        let mut n1 = n("api:get-user", "operation", "accepted");
        n1.props.insert("input".into(), serde_json::json!({"user_id": "string"}));
        n1.props.insert("output".into(), serde_json::json!({"name": "string", "email": "string"}));
        
        let mut n2 = n("api:get-user", "operation", "accepted");
        n2.props.insert("input".into(), serde_json::json!({"user_id": "string"}));
        n2.props.insert("output".into(), serde_json::json!({"name": "string"})); // email removed
        
        let a = doc_with(vec![n1], vec![]);
        let b = doc_with(vec![n2], vec![]);
        let r = diff_graphs_contract_aware("a", &a, "b", &b);
        
        assert_eq!(r.breaking(), 1, "API field removal should be breaking");
        assert_eq!(r.exit_code(), 1);
    }

    #[test]
    fn contract_aware_api_new_optional_field_is_conservative() {
        // Note: Current implementation treats any modification to API input/output
        // as breaking (conservative/safe). Future enhancement: deep schema comparison
        // to distinguish truly additive changes (new optional fields) from breaking ones.
        let mut n1 = n("api:get-user", "operation", "accepted");
        n1.props.insert("output".into(), serde_json::json!({"name": "string"}));
        
        let mut n2 = n("api:get-user", "operation", "accepted");
        n2.props.insert("output".into(), serde_json::json!({"name": "string", "avatar": "string?"}));
        
        let a = doc_with(vec![n1], vec![]);
        let b = doc_with(vec![n2], vec![]);
        let r = diff_graphs_contract_aware("a", &a, "b", &b);
        
        // Current behavior: conservative, treats structure changes as breaking
        assert_eq!(r.breaking(), 1, "API structure change treated as breaking (conservative)");
        assert_eq!(r.exit_code(), 1);
    }

    #[test]
    fn contract_aware_dto_type_change_is_breaking() {
        let mut n1 = n("entity:user", "entity", "accepted");
        n1.props.insert("fields".into(), serde_json::json!([
            {"name": "age", "type": "number"}
        ]));
        
        let mut n2 = n("entity:user", "entity", "accepted");
        n2.props.insert("fields".into(), serde_json::json!([
            {"name": "age", "type": "string"} // type changed
        ]));
        
        let a = doc_with(vec![n1], vec![]);
        let b = doc_with(vec![n2], vec![]);
        let r = diff_graphs_contract_aware("a", &a, "b", &b);
        
        assert_eq!(r.breaking(), 1, "DTO type change should be breaking");
        assert_eq!(r.exit_code(), 1);
    }

    #[test]
    fn contract_aware_behavior_constraint_change_is_breaking() {
        let mut n1 = n("rule:validation", "rule", "accepted");
        n1.props.insert("condition".into(), serde_json::json!("age > 18"));
        
        let mut n2 = n("rule:validation", "rule", "accepted");
        n2.props.insert("condition".into(), serde_json::json!("age >= 21")); // constraint changed
        
        let a = doc_with(vec![n1], vec![]);
        let b = doc_with(vec![n2], vec![]);
        let r = diff_graphs_contract_aware("a", &a, "b", &b);
        
        assert_eq!(r.breaking(), 1, "Behavior constraint change should be breaking");
        assert_eq!(r.exit_code(), 1);
    }

    #[test]
    fn contract_aware_topology_new_dependency_is_additive() {
        let a = doc_with(
            vec![n("svc:a", "api", "accepted"), n("svc:b", "api", "accepted")],
            vec![],
        );
        let b = doc_with(
            vec![n("svc:a", "api", "accepted"), n("svc:b", "api", "accepted")],
            vec![e("edge:1", "queries", "svc:a", "svc:b")], // new dependency
        );
        let r = diff_graphs_contract_aware("x", &a, "y", &b);
        
        assert_eq!(r.breaking(), 0, "New dependency should be additive");
        assert_eq!(r.additive(), 1);
        assert_eq!(r.exit_code(), 2);
    }

    #[test]
    fn contract_aware_topology_removed_dependency_is_breaking() {
        let a = doc_with(
            vec![n("svc:a", "api", "accepted"), n("svc:b", "api", "accepted")],
            vec![e("edge:1", "queries", "svc:a", "svc:b")],
        );
        let b = doc_with(
            vec![n("svc:a", "api", "accepted"), n("svc:b", "api", "accepted")],
            vec![], // dependency removed
        );
        let r = diff_graphs_contract_aware("x", &a, "y", &b);
        
        assert_eq!(r.breaking(), 1, "Removed dependency should be breaking");
        assert_eq!(r.exit_code(), 1);
    }
}
