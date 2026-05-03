use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerCounts {
    pub questions: usize,
    pub decisions: usize,
    pub evidence: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerIndex {
    pub version: String,
    pub updated_at: DateTime<Utc>,
    pub counts: LedgerCounts,
}

impl LedgerIndex {
    pub fn new() -> Self {
        Self {
            version: "1.0.0".to_string(),
            updated_at: Utc::now(),
            counts: LedgerCounts {
                questions: 0,
                decisions: 0,
                evidence: 0,
            },
        }
    }

    pub fn update_counts(&mut self, questions: usize, decisions: usize, evidence: usize) {
        self.counts.questions = questions;
        self.counts.decisions = decisions;
        self.counts.evidence = evidence;
        self.updated_at = Utc::now();
    }
}

impl Default for LedgerIndex {
    fn default() -> Self {
        Self::new()
    }
}
