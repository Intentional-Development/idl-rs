use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuestionStatus {
    Open,
    Answered,
    Abandoned,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: Uuid,
    pub ts: DateTime<Utc>,
    pub asked_by: String,
    pub topic: String,
    pub body: String,
    #[serde(default)]
    pub blocks_decisions: Vec<Uuid>,
    pub status: QuestionStatus,
    pub answered_by_decision: Option<Uuid>,
}

impl Question {
    pub fn new(
        asked_by: String,
        topic: String,
        body: String,
        blocks_decisions: Vec<Uuid>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            ts: Utc::now(),
            asked_by,
            topic,
            body,
            blocks_decisions,
            status: QuestionStatus::Open,
            answered_by_decision: None,
        }
    }

    pub fn answer(&mut self, decision_id: Uuid) {
        self.status = QuestionStatus::Answered;
        self.answered_by_decision = Some(decision_id);
    }

    pub fn abandon(&mut self) {
        self.status = QuestionStatus::Abandoned;
    }
}
