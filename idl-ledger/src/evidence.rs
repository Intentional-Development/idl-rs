use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceKind {
    Commit,
    TestRun,
    AuditLog,
    ExternalUrl,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub id: Uuid,
    pub ts: DateTime<Utc>,
    pub kind: EvidenceKind,
    pub uri: String,
    pub hash: Option<String>,
    pub captured_by: String,
    pub supports_decision: Option<Uuid>,
    pub supports_question: Option<Uuid>,
}

impl Evidence {
    pub fn new(
        kind: EvidenceKind,
        uri: String,
        captured_by: String,
        hash: Option<String>,
        supports_decision: Option<Uuid>,
        supports_question: Option<Uuid>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            ts: Utc::now(),
            kind,
            uri,
            hash,
            captured_by,
            supports_decision,
            supports_question,
        }
    }
}
