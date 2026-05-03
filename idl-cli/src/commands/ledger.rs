use anyhow::{Context, Result};
use idl_ledger::{
    get_ledger_dir, Decision, DecisionScope, Evidence, EvidenceKind, LedgerStorage, LedgerValidator,
    Question, QuestionStatus,
};
use std::process::ExitCode;
use uuid::Uuid;

pub fn run_ask(
    body: String,
    topic: Option<String>,
    blocks: Vec<Uuid>,
    json: bool,
    dry_run: bool,
) -> Result<ExitCode> {
    let storage = LedgerStorage::new(get_ledger_dir());
    
    let asked_by = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
    let topic = topic.unwrap_or_else(|| "general".to_string());
    
    let question = Question::new(asked_by, topic, body, blocks);
    
    if json {
        println!("{}", serde_json::to_string_pretty(&question)?);
    } else {
        println!("Question created: {}", question.id);
        println!("  Topic: {}", question.topic);
        println!("  Asked by: {}", question.asked_by);
        println!("  Status: {:?}", question.status);
    }
    
    if !dry_run {
        storage.save_question(&question)
            .context("Failed to save question")?;
        if !json {
            println!("Saved to ledger.");
        }
    } else if !json {
        println!("(Dry run - not saved)");
    }
    
    Ok(ExitCode::SUCCESS)
}

pub fn run_decide(
    title: String,
    rationale: String,
    scope: DecisionScope,
    supersedes: Option<Uuid>,
    evidence: Vec<Uuid>,
    json: bool,
    dry_run: bool,
) -> Result<ExitCode> {
    let storage = LedgerStorage::new(get_ledger_dir());
    
    let decided_by = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
    
    let decision = Decision::new(decided_by, scope, title, rationale, supersedes, evidence);
    
    if json {
        println!("{}", serde_json::to_string_pretty(&decision)?);
    } else {
        println!("Decision created: {}", decision.id);
        println!("  Title: {}", decision.title);
        println!("  Scope: {:?}", decision.scope);
        println!("  Decided by: {}", decision.decided_by);
    }
    
    if !dry_run {
        storage.save_decision(&decision)
            .context("Failed to save decision")?;
        if !json {
            println!("Saved to ledger.");
        }
    } else if !json {
        println!("(Dry run - not saved)");
    }
    
    Ok(ExitCode::SUCCESS)
}

pub fn run_link(
    kind: EvidenceKind,
    uri: String,
    supports_decision: Option<Uuid>,
    supports_question: Option<Uuid>,
    hash: Option<String>,
    json: bool,
    dry_run: bool,
) -> Result<ExitCode> {
    let storage = LedgerStorage::new(get_ledger_dir());
    
    let captured_by = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
    
    let evidence = Evidence::new(
        kind,
        uri,
        captured_by,
        hash,
        supports_decision,
        supports_question,
    );
    
    if json {
        println!("{}", serde_json::to_string_pretty(&evidence)?);
    } else {
        println!("Evidence created: {}", evidence.id);
        println!("  Kind: {:?}", evidence.kind);
        println!("  URI: {}", evidence.uri);
        println!("  Captured by: {}", evidence.captured_by);
    }
    
    if !dry_run {
        storage.save_evidence(&evidence)
            .context("Failed to save evidence")?;
        if !json {
            println!("Saved to ledger.");
        }
    } else if !json {
        println!("(Dry run - not saved)");
    }
    
    Ok(ExitCode::SUCCESS)
}

pub fn run_list(
    kind: Option<String>,
    status: Option<String>,
    json: bool,
) -> Result<ExitCode> {
    let storage = LedgerStorage::new(get_ledger_dir());
    
    match kind.as_deref() {
        Some("question") => {
            let questions = storage.list_questions()?;
            let filtered: Vec<_> = if let Some(status) = status {
                questions.into_iter()
                    .filter(|q| match status.as_str() {
                        "open" => q.status == QuestionStatus::Open,
                        "answered" => q.status == QuestionStatus::Answered,
                        "abandoned" => q.status == QuestionStatus::Abandoned,
                        _ => true,
                    })
                    .collect()
            } else {
                questions
            };
            
            if json {
                println!("{}", serde_json::to_string_pretty(&filtered)?);
            } else {
                println!("Questions ({})", filtered.len());
                for q in filtered {
                    println!("  {} | {} | {:?} | {}", q.id, q.topic, q.status, q.body.chars().take(60).collect::<String>());
                }
            }
        }
        Some("decision") => {
            let decisions = storage.list_decisions()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&decisions)?);
            } else {
                println!("Decisions ({})", decisions.len());
                for d in decisions {
                    println!("  {} | {:?} | {}", d.id, d.scope, d.title);
                }
            }
        }
        Some("evidence") => {
            let evidence = storage.list_evidence()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&evidence)?);
            } else {
                println!("Evidence ({})", evidence.len());
                for e in evidence {
                    println!("  {} | {:?} | {}", e.id, e.kind, e.uri);
                }
            }
        }
        None => {
            let questions = storage.list_questions()?;
            let decisions = storage.list_decisions()?;
            let evidence = storage.list_evidence()?;
            
            if json {
                let summary = serde_json::json!({
                    "questions": questions.len(),
                    "decisions": decisions.len(),
                    "evidence": evidence.len(),
                });
                println!("{}", serde_json::to_string_pretty(&summary)?);
            } else {
                println!("Ledger Summary:");
                println!("  Questions: {}", questions.len());
                println!("  Decisions: {}", decisions.len());
                println!("  Evidence: {}", evidence.len());
            }
        }
        Some(k) => {
            anyhow::bail!("Unknown kind: {}. Valid kinds: question, decision, evidence", k);
        }
    }
    
    Ok(ExitCode::SUCCESS)
}

pub fn run_validate(json: bool) -> Result<ExitCode> {
    let storage = LedgerStorage::new(get_ledger_dir());
    let validator = LedgerValidator::new(storage);
    
    let report = validator.validate()?;
    
    if json {
        let output = serde_json::json!({
            "valid": report.is_valid(),
            "errors": report.errors,
            "counts": {
                "questions": report.questions_count,
                "decisions": report.decisions_count,
                "evidence": report.evidence_count,
            }
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("Ledger Validation Report");
        println!("========================");
        println!("Questions: {}", report.questions_count);
        println!("Decisions: {}", report.decisions_count);
        println!("Evidence: {}", report.evidence_count);
        println!();
        
        if report.is_valid() {
            println!("✓ All references are valid.");
        } else {
            println!("✗ Found {} error(s):", report.errors.len());
            for (i, err) in report.errors.iter().enumerate() {
                println!("  {}. {}", i + 1, err);
            }
        }
    }
    
    if report.is_valid() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

pub fn run_init() -> Result<ExitCode> {
    let storage = LedgerStorage::new(get_ledger_dir());
    storage.init().context("Failed to initialize ledger")?;
    println!("Ledger initialized at {}", get_ledger_dir().display());
    Ok(ExitCode::SUCCESS)
}
