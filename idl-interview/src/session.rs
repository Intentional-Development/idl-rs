//! Session storage: `intent/.idl/interview/sessions/<id>/{session.json, round-N.json, round-N.md}`.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Running,
    Completed,
    Accepted,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Round {
    pub n: u32,
    pub transcript_md: String,
    pub graph_delta_json: Value,
    #[serde(default)]
    pub questions: Vec<Value>,
    #[serde(default)]
    pub decisions: Vec<Value>,
    pub confidence_overall: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub topic: String,
    pub started_at: String,
    pub model: String,
    #[serde(default)]
    pub rounds_planned: u32,
    pub status: SessionStatus,
    #[serde(default)]
    pub rounds: Vec<Round>,
    #[serde(default)]
    pub promoted_change_id: Option<String>,
    /// Filesystem root; not serialized — set by `load`/`new`.
    #[serde(skip)]
    pub root: PathBuf,
}

impl Session {
    /// Build a fresh session with a deterministic id under `intent_dir`.
    pub fn new(
        intent_dir: &Path,
        topic: impl Into<String>,
        model: impl Into<String>,
        rounds_planned: u32,
    ) -> Result<Self> {
        let id = generate_session_id();
        let root = sessions_root(intent_dir).join(&id);
        std::fs::create_dir_all(&root).with_context(|| format!("create {}", root.display()))?;
        let s = Session {
            id,
            topic: topic.into(),
            started_at: iso_now(),
            model: model.into(),
            rounds_planned,
            status: SessionStatus::Running,
            rounds: vec![],
            promoted_change_id: None,
            root,
        };
        s.save()?;
        Ok(s)
    }

    pub fn load(intent_dir: &Path, id: &str) -> Result<Self> {
        let root = sessions_root(intent_dir).join(id);
        let path = root.join("session.json");
        let text =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let mut s: Session = serde_json::from_str(&text).context("parse session.json")?;
        s.root = root;
        // Re-load rounds from disk to keep storage as source of truth.
        s.rounds.clear();
        for n in 1..=s.rounds_planned {
            let rp = s.root.join(format!("round-{n}.json"));
            if !rp.exists() {
                break;
            }
            let r_text = std::fs::read_to_string(&rp)?;
            let r: Round =
                serde_json::from_str(&r_text).with_context(|| format!("parse {}", rp.display()))?;
            s.rounds.push(r);
        }
        Ok(s)
    }

    pub fn save(&self) -> Result<()> {
        let path = self.root.join("session.json");
        let value = serde_json::to_value(self)?;
        std::fs::write(&path, serde_json::to_string_pretty(&value)? + "\n")
            .with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    pub fn write_round(&mut self, r: Round) -> Result<()> {
        let json_path = self.root.join(format!("round-{}.json", r.n));
        let md_path = self.root.join(format!("round-{}.md", r.n));
        std::fs::write(&json_path, serde_json::to_string_pretty(&r)? + "\n")?;
        std::fs::write(&md_path, &r.transcript_md)?;
        // Replace any prior round with this number, then append.
        self.rounds.retain(|x| x.n != r.n);
        self.rounds.push(r);
        self.rounds.sort_by_key(|x| x.n);
        self.save()
    }

    /// Accumulate per-round deltas into a single `{version, nodes, edges}` graph.
    /// Later rounds win on duplicate ids (kernel "supersedes" semantics).
    pub fn current_graph(&self) -> Value {
        let mut nodes_by_id: std::collections::BTreeMap<String, Value> = Default::default();
        let mut edges_by_id: std::collections::BTreeMap<String, Value> = Default::default();
        for r in &self.rounds {
            if let Some(arr) = r.graph_delta_json.get("nodes").and_then(Value::as_array) {
                for n in arr {
                    if let Some(id) = n.get("id").and_then(Value::as_str) {
                        nodes_by_id.insert(id.to_string(), n.clone());
                    }
                }
            }
            if let Some(arr) = r.graph_delta_json.get("edges").and_then(Value::as_array) {
                for e in arr {
                    if let Some(id) = e.get("id").and_then(Value::as_str) {
                        edges_by_id.insert(id.to_string(), e.clone());
                    }
                }
            }
        }
        json!({
            "version": "0.1.0",
            "metadata": {
                "session_id": self.id,
                "topic": self.topic,
                "rounds": self.rounds.len()
            },
            "nodes": nodes_by_id.into_values().collect::<Vec<_>>(),
            "edges": edges_by_id.into_values().collect::<Vec<_>>()
        })
    }

    pub fn list(intent_dir: &Path) -> Result<Vec<Session>> {
        let root = sessions_root(intent_dir);
        if !root.is_dir() {
            return Ok(vec![]);
        }
        let mut out = vec![];
        for e in std::fs::read_dir(&root)? {
            let e = e?;
            if !e.path().is_dir() {
                continue;
            }
            let id = e.file_name().to_string_lossy().into_owned();
            match Self::load(intent_dir, &id) {
                Ok(s) => out.push(s),
                Err(err) => tracing::warn!("skip session {id}: {err}"),
            }
        }
        out.sort_by(|a, b| a.started_at.cmp(&b.started_at));
        Ok(out)
    }

    pub fn next_round_number(&self) -> u32 {
        self.rounds.iter().map(|r| r.n).max().unwrap_or(0) + 1
    }
}

pub fn sessions_root(intent_dir: &Path) -> PathBuf {
    intent_dir.join(".idl").join("interview").join("sessions")
}

fn generate_session_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("sess-{secs:010}-{:06x}", nanos & 0xFF_FFFF)
}

fn iso_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_unix_seconds(secs)
}

fn format_unix_seconds(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let secs_today = (secs % 86_400) as u32;
    let (y, m, d) = days_to_ymd(days);
    let hh = secs_today / 3600;
    let mm = (secs_today % 3600) / 60;
    let ss = secs_today % 60;
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
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

/// Find the nearest `intent/` directory walking upward from `start`.
pub fn locate_intent_dir(start: &Path) -> Result<PathBuf> {
    let mut cur = start.to_path_buf();
    loop {
        let candidate = cur.join("intent");
        if candidate.is_dir() {
            return Ok(candidate);
        }
        if !cur.pop() {
            break;
        }
    }
    Err(anyhow!(
        "no `intent/` directory found from {} — run `idl init --greenfield` first",
        start.display()
    ))
}
