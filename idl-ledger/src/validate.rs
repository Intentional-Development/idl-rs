use crate::{LedgerStorage, Result};
use std::collections::HashSet;
use uuid::Uuid;

pub struct LedgerValidator {
    storage: LedgerStorage,
}

impl LedgerValidator {
    pub fn new(storage: LedgerStorage) -> Self {
        Self { storage }
    }

    pub fn validate(&self) -> Result<ValidationReport> {
        let mut report = ValidationReport::default();

        let questions = self.storage.list_questions()?;
        let decisions = self.storage.list_decisions()?;
        let evidence = self.storage.list_evidence()?;

        let decision_ids: HashSet<Uuid> = decisions.iter().map(|d| d.id).collect();
        let evidence_ids: HashSet<Uuid> = evidence.iter().map(|e| e.id).collect();
        let question_ids: HashSet<Uuid> = questions.iter().map(|q| q.id).collect();

        // Validate question references
        for question in &questions {
            for decision_id in &question.blocks_decisions {
                if !decision_ids.contains(decision_id) {
                    report.add_error(format!(
                        "Question {} references non-existent decision {}",
                        question.id, decision_id
                    ));
                }
            }
            if let Some(decision_id) = question.answered_by_decision {
                if !decision_ids.contains(&decision_id) {
                    report.add_error(format!(
                        "Question {} answered by non-existent decision {}",
                        question.id, decision_id
                    ));
                }
            }
        }

        // Validate decision references
        for decision in &decisions {
            if let Some(supersedes_id) = decision.supersedes {
                if !decision_ids.contains(&supersedes_id) {
                    report.add_error(format!(
                        "Decision {} supersedes non-existent decision {}",
                        decision.id, supersedes_id
                    ));
                }
            }
            for evidence_id in &decision.evidence_refs {
                if !evidence_ids.contains(evidence_id) {
                    report.add_error(format!(
                        "Decision {} references non-existent evidence {}",
                        decision.id, evidence_id
                    ));
                }
            }
        }

        // Validate evidence references
        for evid in &evidence {
            if let Some(decision_id) = evid.supports_decision {
                if !decision_ids.contains(&decision_id) {
                    report.add_error(format!(
                        "Evidence {} supports non-existent decision {}",
                        evid.id, decision_id
                    ));
                }
            }
            if let Some(question_id) = evid.supports_question {
                if !question_ids.contains(&question_id) {
                    report.add_error(format!(
                        "Evidence {} supports non-existent question {}",
                        evid.id, question_id
                    ));
                }
            }
        }

        report.questions_count = questions.len();
        report.decisions_count = decisions.len();
        report.evidence_count = evidence.len();

        Ok(report)
    }
}

#[derive(Debug, Default)]
pub struct ValidationReport {
    pub errors: Vec<String>,
    pub questions_count: usize,
    pub decisions_count: usize,
    pub evidence_count: usize,
}

impl ValidationReport {
    fn add_error(&mut self, error: String) {
        self.errors.push(error);
    }

    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}
