//! `accept` flow: promote the latest accumulated delta into a proposed
//! `intent/changes/NNNN-<slug>/` change folder.
//!
//! The folder layout mirrors `cli-spec.md`:
//!
//! ```text
//! intent/changes/NNNN-<slug>/
//!   state.json         (state=proposed)
//!   intent-delta.idl   (placeholder; the canonical delta lives in delta.json)
//!   delta.json         (the full accumulated kernel-conformant graph)
//!   decisions.md       (rendered decisions ledger)
//!   sources.json       (one entry per source anchor)
//!   verifications/plan.md
//!   ai-runs/<run-id>.jsonl
//! ```
//!
//! `intent/project.idl` is never mutated by accept; promotion of the kernel
//! graph into the accepted set remains the responsibility of `idl change accept`.

use crate::session::Session;
use anyhow::{anyhow, Context, Result};
use idl_graph::{EdgeKind, NodeKind};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub struct AcceptOutcome {
    pub change_id: String,
    pub folder: PathBuf,
    pub node_count: usize,
    pub edge_count: usize,
}

pub fn accept(intent_dir: &Path, session: &mut Session) -> Result<AcceptOutcome> {
    let graph = session.current_graph();
    validate_kernel(&graph)?;

    let next = next_change_number(intent_dir)?;
    let slug = derive_slug(&session.topic);
    let change_id = format!("{next:04}-{slug}");
    let folder = intent_dir.join("changes").join(&change_id);
    if folder.exists() {
        return Err(anyhow!("change folder {} already exists", folder.display()));
    }
    std::fs::create_dir_all(folder.join("ai-runs"))?;
    std::fs::create_dir_all(folder.join("verifications"))?;

    let now = iso_now();
    let state_json = json!({
        "id": change_id,
        "state": "proposed",
        "created_at": now,
        "updated_at": now,
        "transitions": [{ "from": "draft", "to": "proposed", "at": now, "by": "idl-interview" }],
        "source": {
            "kind": "interview",
            "session_id": session.id
        }
    });
    write_pretty(&folder.join("state.json"), &state_json)?;

    write_pretty(&folder.join("delta.json"), &graph)?;
    std::fs::write(
        folder.join("intent-delta.idl"),
        format!(
            "# Proposed delta from interview session {}\n\
             # Topic: {}\n\
             # Canonical kernel graph: ./delta.json\n",
            session.id, session.topic
        ),
    )?;

    let decisions_md = render_decisions_md(session);
    std::fs::write(folder.join("decisions.md"), decisions_md)?;

    let sources = collect_sources(&graph);
    write_pretty(&folder.join("sources.json"), &sources)?;

    let plan_md = render_verification_plan(&graph);
    std::fs::write(folder.join("verifications").join("plan.md"), plan_md)?;

    let run_path = folder.join("ai-runs").join(format!("{}.jsonl", session.id));
    let mut run_lines = String::new();
    for r in &session.rounds {
        let line = json!({
            "round": r.n,
            "confidence_overall": r.confidence_overall,
            "questions": r.questions,
            "decisions": r.decisions,
        });
        run_lines.push_str(&line.to_string());
        run_lines.push('\n');
    }
    std::fs::write(run_path, run_lines)?;

    session.promoted_change_id = Some(change_id.clone());
    session.status = crate::session::SessionStatus::Accepted;
    session.save()?;

    let nodes = graph
        .get("nodes")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let edges = graph
        .get("edges")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);

    Ok(AcceptOutcome {
        change_id,
        folder,
        node_count: nodes,
        edge_count: edges,
    })
}

fn validate_kernel(graph: &Value) -> Result<()> {
    let nodes = graph
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("graph missing nodes"))?;
    if nodes.is_empty() {
        return Err(anyhow!("refuse to accept empty graph"));
    }
    for n in nodes {
        let kind = n
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("node missing kind"))?;
        NodeKind::from_str(kind)
            .map_err(|_| anyhow!("non-kernel node kind `{kind}` cannot be accepted"))?;
        if n.get("state").and_then(Value::as_str) != Some("proposed") {
            return Err(anyhow!(
                "node `{}` must be state=proposed",
                n.get("id").and_then(Value::as_str).unwrap_or("?")
            ));
        }
        if n.get("confidence").is_none() {
            return Err(anyhow!(
                "node `{}` missing confidence",
                n.get("id").and_then(Value::as_str).unwrap_or("?")
            ));
        }
    }
    if let Some(edges) = graph.get("edges").and_then(Value::as_array) {
        for e in edges {
            let kind = e
                .get("kind")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("edge missing kind"))?;
            EdgeKind::from_str(kind).map_err(|_| anyhow!("non-kernel edge kind `{kind}`"))?;
        }
    }
    Ok(())
}

fn next_change_number(intent_dir: &Path) -> Result<u32> {
    let changes = intent_dir.join("changes");
    if !changes.is_dir() {
        std::fs::create_dir_all(&changes)
            .with_context(|| format!("create {}", changes.display()))?;
        return Ok(1);
    }
    let mut max = 0u32;
    for e in std::fs::read_dir(&changes)? {
        let e = e?;
        if !e.path().is_dir() {
            continue;
        }
        let name = e.file_name();
        let name = name.to_string_lossy();
        let prefix: String = name.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(n) = prefix.parse::<u32>() {
            max = max.max(n);
        }
    }
    Ok(max + 1)
}

fn derive_slug(topic: &str) -> String {
    let mut s = String::new();
    for c in topic.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if (c.is_whitespace() || c == '-' || c == '_') && !s.ends_with('-') && !s.is_empty()
        {
            s.push('-');
        }
    }
    while s.ends_with('-') {
        s.pop();
    }
    if s.is_empty() {
        s = "interview".into();
    }
    s.truncate(48);
    s
}

fn render_decisions_md(session: &Session) -> String {
    let mut out = String::from("# Decisions\n\n");
    for r in &session.rounds {
        if r.decisions.is_empty() {
            continue;
        }
        out.push_str(&format!("## Round {}\n\n", r.n));
        for d in &r.decisions {
            let id = d.get("id").and_then(Value::as_str).unwrap_or("?");
            let q = d
                .get("question_ref")
                .and_then(Value::as_str)
                .unwrap_or("(root)");
            let a = d.get("answer").and_then(Value::as_str).unwrap_or("");
            out.push_str(&format!("- **{id}** answers `{q}` → {a}\n"));
        }
        out.push('\n');
    }
    out
}

fn collect_sources(graph: &Value) -> Value {
    let mut anchors: Vec<Value> = vec![];
    if let Some(nodes) = graph.get("nodes").and_then(Value::as_array) {
        for n in nodes {
            if let Some(arr) = n.get("source_anchors").and_then(Value::as_array) {
                for a in arr {
                    anchors.push(a.clone());
                }
            }
        }
    }
    json!({ "anchors": anchors })
}

fn render_verification_plan(graph: &Value) -> String {
    let mut out = String::from("# Verification Plan\n\n");
    if let Some(nodes) = graph.get("nodes").and_then(Value::as_array) {
        for n in nodes {
            if n.get("kind").and_then(Value::as_str) == Some("verification") {
                let id = n.get("id").and_then(Value::as_str).unwrap_or("?");
                let name = n
                    .get("props")
                    .and_then(|p| p.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or(id);
                out.push_str(&format!("- {name} ({id}) — status: unknown\n"));
            }
        }
    }
    out
}

fn write_pretty(path: &Path, v: &Value) -> Result<()> {
    std::fs::write(path, serde_json::to_string_pretty(v)? + "\n")
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn iso_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Defer to a tiny local formatter — duplicated from session.rs to keep
    // this module standalone-buildable.
    let days = (secs / 86_400) as i64;
    let st = (secs % 86_400) as u32;
    let (y, m, d) = days_to_ymd(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        st / 3600,
        (st % 3600) / 60,
        st % 60
    )
}

fn days_to_ymd(mut days: i64) -> (i32, u32, u32) {
    days += 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let doe = (days - era * 146_097) as u32;
    let yoe = (doe
        .wrapping_sub(doe / 1460)
        .wrapping_sub(doe / 36_524)
        .wrapping_add(doe / 146_096))
        / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = y + if m <= 2 { 1 } else { 0 };
    (y as i32, m, d)
}
