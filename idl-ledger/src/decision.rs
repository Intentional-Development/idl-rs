use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DecisionScope {
    Architecture,
    Process,
    Scope,
    Tooling,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub id: Uuid,
    pub ts: DateTime<Utc>,
    pub decided_by: String,
    pub scope: DecisionScope,
    pub title: String,
    pub body: String,
    pub supersedes: Option<Uuid>,
    #[serde(default)]
    pub evidence_refs: Vec<Uuid>,
}

impl Decision {
    pub fn new(
        decided_by: String,
        scope: DecisionScope,
        title: String,
        body: String,
        supersedes: Option<Uuid>,
        evidence_refs: Vec<Uuid>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            ts: Utc::now(),
            decided_by,
            scope,
            title,
            body,
            supersedes,
            evidence_refs,
        }
    }

    pub fn add_evidence(&mut self, evidence_id: Uuid) {
        if !self.evidence_refs.contains(&evidence_id) {
            self.evidence_refs.push(evidence_id);
        }
    }
}
