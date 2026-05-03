#[cfg(test)]
mod tests {
    use crate::{LedgerStorage, LedgerValidator, Question, Decision, DecisionScope, Evidence, EvidenceKind};
    use tempfile::TempDir;
    use uuid::Uuid;

    #[test]
    fn test_validate_empty_ledger() {
        let temp = TempDir::new().unwrap();
        let storage = LedgerStorage::new(temp.path());
        storage.init().unwrap();
        
        let validator = LedgerValidator::new(storage);
        let report = validator.validate().unwrap();
        
        assert!(report.is_valid());
        assert_eq!(report.errors.len(), 0);
    }

    #[test]
    fn test_validate_valid_references() {
        let temp = TempDir::new().unwrap();
        let storage = LedgerStorage::new(temp.path());
        storage.init().unwrap();
        
        // Create a decision
        let d = Decision::new(
            "stark".to_string(),
            DecisionScope::Architecture,
            "Test decision".to_string(),
            "Rationale".to_string(),
            None,
            vec![],
        );
        let decision_id = d.id;
        storage.save_decision(&d).unwrap();
        
        // Create a question that references it
        let q = Question::new(
            "banner".to_string(),
            "test".to_string(),
            "Question".to_string(),
            vec![decision_id],
        );
        storage.save_question(&q).unwrap();
        
        let validator = LedgerValidator::new(storage);
        let report = validator.validate().unwrap();
        
        assert!(report.is_valid());
    }

    #[test]
    fn test_validate_invalid_question_reference() {
        let temp = TempDir::new().unwrap();
        let storage = LedgerStorage::new(temp.path());
        storage.init().unwrap();
        
        let fake_decision = Uuid::new_v4();
        let q = Question::new(
            "banner".to_string(),
            "test".to_string(),
            "Question".to_string(),
            vec![fake_decision],
        );
        storage.save_question(&q).unwrap();
        
        let validator = LedgerValidator::new(storage);
        let report = validator.validate().unwrap();
        
        assert!(!report.is_valid());
        assert_eq!(report.errors.len(), 1);
        assert!(report.errors[0].contains("non-existent decision"));
    }

    #[test]
    fn test_validate_invalid_evidence_reference() {
        let temp = TempDir::new().unwrap();
        let storage = LedgerStorage::new(temp.path());
        storage.init().unwrap();
        
        let fake_decision = Uuid::new_v4();
        let e = Evidence::new(
            EvidenceKind::Commit,
            "abc123".to_string(),
            "barton".to_string(),
            None,
            Some(fake_decision),
            None,
        );
        storage.save_evidence(&e).unwrap();
        
        let validator = LedgerValidator::new(storage);
        let report = validator.validate().unwrap();
        
        assert!(!report.is_valid());
        assert!(report.errors[0].contains("non-existent decision"));
    }
}
