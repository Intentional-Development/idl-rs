//! AI run audit logging for IDL mutations.
//!
//! This module provides a structured audit trail for all LLM-driven mutations
//! on IDL state. Every mutation (proposal creation, acceptance, rejection, MCP tool call)
//! is logged to `.idl/ai-run.jsonl` as a single JSON object per line.
//!
//! ## Features
//! - Append-only JSONL format (one event per line)
//! - File locking for concurrent writes
//! - Configurable via `IDL_AUDIT_LOG` env var
//! - Schema-validated events (see `schemas/ai-run-event.schema.json`)
//! - Hash tracking for spec mutations
//!
//! ## Usage
//! ```no_run
//! use idl_audit::{AuditWriter, AuditEvent, Actor};
//!
//! let writer = AuditWriter::new(None)?;
//! let event = AuditEvent::builder()
//!     .actor(Actor::Cli)
//!     .tool("propose.create")
//!     .target("proposal:20260503-add-field")
//!     .outcome_success()
//!     .build()?;
//! writer.log(&event)?;
//! # Ok::<(), anyhow::Error>(())
//! ```

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Actor type for audit events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Actor {
    Agent,
    Human,
    Cli,
}

/// Outcome of a mutation attempt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Success,
    Error,
    Rejected,
}

/// An audit event representing a mutation attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// ISO 8601 UTC timestamp
    pub ts: DateTime<Utc>,
    /// UUID grouping multi-step runs
    pub run_id: Uuid,
    /// Actor type (agent, human, cli)
    pub actor: Actor,
    /// Agent name if actor is agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// LLM model identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// SHA256 hash of prompt (with "sha256:" prefix)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_hash: Option<String>,
    /// Tool or command invoked
    pub tool: String,
    /// Structured arguments (secrets redacted)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args_redacted: Option<Value>,
    /// Target entity being mutated
    pub target: String,
    /// Outcome of the mutation
    pub outcome: Outcome,
    /// Error message if outcome is error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// SHA256 hash of spec before mutation (with "sha256:" prefix)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec_before_hash: Option<String>,
    /// SHA256 hash of spec after mutation (with "sha256:" prefix)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec_after_hash: Option<String>,
}

/// Builder for audit events.
pub struct AuditEventBuilder {
    run_id: Uuid,
    actor: Option<Actor>,
    agent: Option<String>,
    model: Option<String>,
    prompt_hash: Option<String>,
    tool: Option<String>,
    args_redacted: Option<Value>,
    target: Option<String>,
    outcome: Option<Outcome>,
    error: Option<String>,
    spec_before_hash: Option<String>,
    spec_after_hash: Option<String>,
}

impl AuditEvent {
    /// Create a new builder for an audit event.
    pub fn builder() -> AuditEventBuilder {
        AuditEventBuilder::new()
    }
}

impl AuditEventBuilder {
    pub fn new() -> Self {
        Self {
            run_id: Uuid::new_v4(),
            actor: None,
            agent: None,
            model: None,
            prompt_hash: None,
            tool: None,
            args_redacted: None,
            target: None,
            outcome: None,
            error: None,
            spec_before_hash: None,
            spec_after_hash: None,
        }
    }

    /// Set the run ID (defaults to new UUID if not set).
    pub fn run_id(mut self, run_id: Uuid) -> Self {
        self.run_id = run_id;
        self
    }

    /// Set the actor type.
    pub fn actor(mut self, actor: Actor) -> Self {
        self.actor = Some(actor);
        self
    }

    /// Set the agent name.
    pub fn agent(mut self, agent: impl Into<String>) -> Self {
        self.agent = Some(agent.into());
        self
    }

    /// Set the LLM model.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set the prompt hash (automatically prefixes with "sha256:").
    pub fn prompt_hash(mut self, hash: impl Into<String>) -> Self {
        let hash = hash.into();
        self.prompt_hash = Some(if hash.starts_with("sha256:") {
            hash
        } else {
            format!("sha256:{}", hash)
        });
        self
    }

    /// Compute and set prompt hash from prompt text.
    pub fn prompt_text(mut self, prompt: &str) -> Self {
        let hash = compute_hash(prompt.as_bytes());
        self.prompt_hash = Some(format!("sha256:{}", hash));
        self
    }

    /// Set the tool name.
    pub fn tool(mut self, tool: impl Into<String>) -> Self {
        self.tool = Some(tool.into());
        self
    }

    /// Set the args (redacted).
    pub fn args(mut self, args: Value) -> Self {
        self.args_redacted = Some(args);
        self
    }

    /// Set the target.
    pub fn target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    /// Set outcome to success.
    pub fn outcome_success(mut self) -> Self {
        self.outcome = Some(Outcome::Success);
        self
    }

    /// Set outcome to error with message.
    pub fn outcome_error(mut self, error: impl Into<String>) -> Self {
        self.outcome = Some(Outcome::Error);
        self.error = Some(error.into());
        self
    }

    /// Set outcome to rejected.
    pub fn outcome_rejected(mut self) -> Self {
        self.outcome = Some(Outcome::Rejected);
        self
    }

    /// Set spec before hash (automatically prefixes with "sha256:").
    pub fn spec_before_hash(mut self, hash: impl Into<String>) -> Self {
        let hash = hash.into();
        self.spec_before_hash = Some(if hash.starts_with("sha256:") {
            hash
        } else {
            format!("sha256:{}", hash)
        });
        self
    }

    /// Set spec after hash (automatically prefixes with "sha256:").
    pub fn spec_after_hash(mut self, hash: impl Into<String>) -> Self {
        let hash = hash.into();
        self.spec_after_hash = Some(if hash.starts_with("sha256:") {
            hash
        } else {
            format!("sha256:{}", hash)
        });
        self
    }

    /// Compute and set spec before hash from content.
    pub fn spec_before(mut self, content: &[u8]) -> Self {
        let hash = compute_hash(content);
        self.spec_before_hash = Some(format!("sha256:{}", hash));
        self
    }

    /// Compute and set spec after hash from content.
    pub fn spec_after(mut self, content: &[u8]) -> Self {
        let hash = compute_hash(content);
        self.spec_after_hash = Some(format!("sha256:{}", hash));
        self
    }

    /// Build the event.
    pub fn build(self) -> Result<AuditEvent> {
        Ok(AuditEvent {
            ts: Utc::now(),
            run_id: self.run_id,
            actor: self.actor.context("actor is required")?,
            agent: self.agent,
            model: self.model,
            prompt_hash: self.prompt_hash,
            tool: self.tool.context("tool is required")?,
            args_redacted: self.args_redacted,
            target: self.target.context("target is required")?,
            outcome: self.outcome.context("outcome is required")?,
            error: self.error,
            spec_before_hash: self.spec_before_hash,
            spec_after_hash: self.spec_after_hash,
        })
    }
}

impl Default for AuditEventBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute SHA256 hash of bytes and return hex digest.
pub fn compute_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Writer for audit events.
pub struct AuditWriter {
    path: PathBuf,
}

impl AuditWriter {
    /// Create a new audit writer.
    ///
    /// If `path` is None, uses `.idl/ai-run.jsonl` or the path from `IDL_AUDIT_LOG` env var.
    pub fn new(path: Option<PathBuf>) -> Result<Self> {
        let path = match path {
            Some(p) => p,
            None => {
                if let Ok(env_path) = std::env::var("IDL_AUDIT_LOG") {
                    PathBuf::from(env_path)
                } else {
                    PathBuf::from(".idl/ai-run.jsonl")
                }
            }
        };

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("create audit log directory {}", parent.display())
            })?;
        }

        Ok(Self { path })
    }

    /// Log an audit event (append-only, with file lock for concurrency safety).
    pub fn log(&self, event: &AuditEvent) -> Result<()> {
        self.log_internal(event, true)
    }

    /// Internal log function with optional locking (used by tests).
    fn log_internal(&self, event: &AuditEvent, use_lock: bool) -> Result<()> {
        let mut line = serde_json::to_string(event).context("serialize audit event")?;
        line.push('\n');

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("open audit log {}", self.path.display()))?;

        if use_lock {
            file.lock_exclusive()
                .with_context(|| format!("acquire lock on {}", self.path.display()))?;
        }

        file.write_all(line.as_bytes())
            .with_context(|| format!("write to audit log {}", self.path.display()))?;

        if use_lock {
            file.unlock()
                .with_context(|| format!("release lock on {}", self.path.display()))?;
        }

        Ok(())
    }

    /// Read all events from the audit log.
    pub fn read_all(&self) -> Result<Vec<AuditEvent>> {
        if !self.path.exists() {
            return Ok(vec![]);
        }

        let content = std::fs::read_to_string(&self.path)
            .with_context(|| format!("read audit log {}", self.path.display()))?;

        let mut events = Vec::new();
        for (idx, line) in content.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let event: AuditEvent = serde_json::from_str(line)
                .with_context(|| format!("parse audit event at line {}", idx + 1))?;
            events.push(event);
        }

        Ok(events)
    }

    /// Get the path to the audit log.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_audit_event_builder() {
        let event = AuditEvent::builder()
            .actor(Actor::Cli)
            .tool("propose.create")
            .target("proposal:20260503-test")
            .outcome_success()
            .build()
            .unwrap();

        assert_eq!(event.actor, Actor::Cli);
        assert_eq!(event.tool, "propose.create");
        assert_eq!(event.target, "proposal:20260503-test");
        assert_eq!(event.outcome, Outcome::Success);
        assert!(event.agent.is_none());
        assert!(event.error.is_none());
    }

    #[test]
    fn test_audit_event_with_agent() {
        let event = AuditEvent::builder()
            .actor(Actor::Agent)
            .agent("stark")
            .model("claude-sonnet-4.5")
            .tool("mcp.proposal.create")
            .target("dto:User")
            .outcome_success()
            .build()
            .unwrap();

        assert_eq!(event.actor, Actor::Agent);
        assert_eq!(event.agent, Some("stark".to_string()));
        assert_eq!(event.model, Some("claude-sonnet-4.5".to_string()));
    }

    #[test]
    fn test_audit_event_with_error() {
        let event = AuditEvent::builder()
            .actor(Actor::Cli)
            .tool("propose.accept")
            .target("proposal:bad-id")
            .outcome_error("proposal not found")
            .build()
            .unwrap();

        assert_eq!(event.outcome, Outcome::Error);
        assert_eq!(event.error, Some("proposal not found".to_string()));
    }

    #[test]
    fn test_audit_event_with_hashes() {
        let before = b"before content";
        let after = b"after content";

        let event = AuditEvent::builder()
            .actor(Actor::Cli)
            .tool("propose.accept")
            .target("graph:project.idl")
            .outcome_success()
            .spec_before(before)
            .spec_after(after)
            .build()
            .unwrap();

        assert!(event.spec_before_hash.is_some());
        assert!(event.spec_after_hash.is_some());
        assert!(event.spec_before_hash.unwrap().starts_with("sha256:"));
        assert!(event.spec_after_hash.unwrap().starts_with("sha256:"));
    }

    #[test]
    fn test_audit_writer_append() {
        let tmp = TempDir::new().unwrap();
        let log_path = tmp.path().join("ai-run.jsonl");

        let writer = AuditWriter::new(Some(log_path.clone())).unwrap();

        // Write first event
        let event1 = AuditEvent::builder()
            .actor(Actor::Cli)
            .tool("propose.create")
            .target("proposal:1")
            .outcome_success()
            .build()
            .unwrap();
        writer.log(&event1).unwrap();

        // Write second event
        let event2 = AuditEvent::builder()
            .actor(Actor::Cli)
            .tool("propose.accept")
            .target("proposal:1")
            .outcome_success()
            .build()
            .unwrap();
        writer.log(&event2).unwrap();

        // Read back
        let events = writer.read_all().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].tool, "propose.create");
        assert_eq!(events[1].tool, "propose.accept");
    }

    #[test]
    fn test_audit_writer_concurrent_writes() {
        use std::sync::Arc;
        use std::thread;

        let tmp = TempDir::new().unwrap();
        let log_path = tmp.path().join("ai-run.jsonl");
        let writer = Arc::new(AuditWriter::new(Some(log_path.clone())).unwrap());

        let mut handles = vec![];
        for i in 0..10 {
            let writer = Arc::clone(&writer);
            let handle = thread::spawn(move || {
                let event = AuditEvent::builder()
                    .actor(Actor::Cli)
                    .tool("test.write")
                    .target(format!("target:{}", i))
                    .outcome_success()
                    .build()
                    .unwrap();
                writer.log(&event).unwrap();
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let events = writer.read_all().unwrap();
        assert_eq!(events.len(), 10);
    }

    #[test]
    fn test_compute_hash() {
        let content = b"test content";
        let hash = compute_hash(content);
        assert_eq!(hash.len(), 64); // SHA256 produces 64 hex chars
    }

    #[test]
    fn test_prompt_text_hash() {
        let prompt = "Create a User DTO with email field";
        let event = AuditEvent::builder()
            .actor(Actor::Agent)
            .agent("stark")
            .tool("mcp.create_dto")
            .target("dto:User")
            .prompt_text(prompt)
            .outcome_success()
            .build()
            .unwrap();

        assert!(event.prompt_hash.is_some());
        let hash = event.prompt_hash.unwrap();
        assert!(hash.starts_with("sha256:"));
        assert_eq!(hash.len(), 71); // "sha256:" (7) + 64 hex chars
    }

    #[test]
    fn test_hash_prefix() {
        let event = AuditEvent::builder()
            .actor(Actor::Cli)
            .tool("test")
            .target("test")
            .outcome_success()
            .spec_before_hash("abc123")
            .build()
            .unwrap();

        assert_eq!(event.spec_before_hash, Some("sha256:abc123".to_string()));

        let event2 = AuditEvent::builder()
            .actor(Actor::Cli)
            .tool("test")
            .target("test")
            .outcome_success()
            .spec_before_hash("sha256:abc123")
            .build()
            .unwrap();

        assert_eq!(
            event2.spec_before_hash,
            Some("sha256:abc123".to_string())
        );
    }

    #[test]
    fn test_serialization() {
        let event = AuditEvent::builder()
            .actor(Actor::Agent)
            .agent("stark")
            .model("claude-sonnet-4.5")
            .tool("mcp.proposal.create")
            .target("dto:User")
            .args(serde_json::json!({"name": "User", "fields": 3}))
            .outcome_success()
            .build()
            .unwrap();

        let json = serde_json::to_string(&event).unwrap();
        let parsed: AuditEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.actor, event.actor);
        assert_eq!(parsed.agent, event.agent);
        assert_eq!(parsed.tool, event.tool);
        assert_eq!(parsed.target, event.target);
    }
}
