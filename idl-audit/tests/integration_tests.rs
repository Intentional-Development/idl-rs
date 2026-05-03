//! Integration tests for AI run audit logging.

use std::sync::Arc;
use std::thread;
use anyhow::Result;
use serde_json::Value;
use tempfile::TempDir;
use idl_audit::{Actor, AuditEvent, AuditWriter, Outcome};

#[test]
fn test_audit_event_schema_compliance() -> Result<()> {
    let tmp = TempDir::new()?;
    let log_path = tmp.path().join("ai-run.jsonl");
    let writer = AuditWriter::new(Some(log_path.clone()))?;

    let event = AuditEvent::builder()
        .actor(Actor::Cli)
        .tool("propose.create")
        .target("proposal:test-123")
        .outcome_success()
        .build()?;
    writer.log(&event)?;

    let events = writer.read_all()?;
    assert_eq!(events.len(), 1);

    let e = &events[0];
    assert_eq!(e.actor, Actor::Cli);
    assert_eq!(e.tool, "propose.create");
    assert_eq!(e.target, "proposal:test-123");
    assert_eq!(e.outcome, Outcome::Success);

    let content = std::fs::read_to_string(&log_path)?;
    let json: Value = serde_json::from_str(content.lines().next().unwrap())?;
    assert!(json.get("ts").is_some());
    assert!(json.get("run_id").is_some());
    Ok(())
}

#[test]
fn test_concurrent_audit_writes() -> Result<()> {
    let tmp = TempDir::new()?;
    let log_path = tmp.path().join("ai-run.jsonl");
    let writer = Arc::new(AuditWriter::new(Some(log_path.clone()))?);

    let mut handles = vec![];
    for i in 0..100 {
        let writer_clone = Arc::clone(&writer);
        let handle = thread::spawn(move || {
            let event = AuditEvent::builder()
                .actor(Actor::Cli)
                .tool(format!("test.write.{}", i))
                .target(format!("target:{}", i))
                .outcome_success()
                .build()
                .unwrap();
            writer_clone.log(&event).unwrap();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let events = writer.read_all()?;
    assert_eq!(events.len(), 100);
    Ok(())
}

#[test]
fn test_jsonl_format_one_per_line() -> Result<()> {
    let tmp = TempDir::new()?;
    let log_path = tmp.path().join("ai-run.jsonl");
    let writer = AuditWriter::new(Some(log_path.clone()))?;

    for i in 0..3 {
        let event = AuditEvent::builder()
            .actor(Actor::Cli)
            .tool(format!("test.{}", i))
            .target(format!("target:{}", i))
            .outcome_success()
            .build()?;
        writer.log(&event)?;
    }

    let content = std::fs::read_to_string(&log_path)?;
    let lines: Vec<_> = content.lines().collect();
    assert_eq!(lines.len(), 3);

    for line in lines {
        let json: Value = serde_json::from_str(line)?;
        assert!(json.is_object());
    }
    Ok(())
}
