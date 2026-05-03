#[cfg(test)]
mod tests {
    use crate::{LedgerStorage, Question, Decision, DecisionScope, Evidence, EvidenceKind};
    use tempfile::TempDir;

    #[test]
    fn test_storage_init() {
        let temp = TempDir::new().unwrap();
        let storage = LedgerStorage::new(temp.path());
        
        storage.init().unwrap();
        
        assert!(temp.path().join("questions").exists());
        assert!(temp.path().join("decisions").exists());
        assert!(temp.path().join("evidence").exists());
        assert!(temp.path().join("index.json").exists());
    }

    #[test]
    fn test_question_round_trip() {
        let temp = TempDir::new().unwrap();
        let storage = LedgerStorage::new(temp.path());
        storage.init().unwrap();
        
        let q = Question::new(
            "banner".to_string(),
            "architecture".to_string(),
            "Test question?".to_string(),
            vec![],
        );
        let id = q.id;
        
        storage.save_question(&q).unwrap();
        let loaded = storage.load_question(id).unwrap();
        
        assert_eq!(loaded.id, id);
        assert_eq!(loaded.body, "Test question?");
    }

    #[test]
    fn test_decision_round_trip() {
        let temp = TempDir::new().unwrap();
        let storage = LedgerStorage::new(temp.path());
        storage.init().unwrap();
        
        let d = Decision::new(
            "stark".to_string(),
            DecisionScope::Tooling,
            "Test decision".to_string(),
            "Rationale here".to_string(),
            None,
            vec![],
        );
        let id = d.id;
        
        storage.save_decision(&d).unwrap();
        let loaded = storage.load_decision(id).unwrap();
        
        assert_eq!(loaded.id, id);
        assert_eq!(loaded.title, "Test decision");
    }

    #[test]
    fn test_evidence_round_trip() {
        let temp = TempDir::new().unwrap();
        let storage = LedgerStorage::new(temp.path());
        storage.init().unwrap();
        
        let e = Evidence::new(
            EvidenceKind::Commit,
            "abc123".to_string(),
            "barton".to_string(),
            None,
            None,
            None,
        );
        let id = e.id;
        
        storage.save_evidence(&e).unwrap();
        let loaded = storage.load_evidence(id).unwrap();
        
        assert_eq!(loaded.id, id);
        assert_eq!(loaded.uri, "abc123");
    }

    #[test]
    fn test_list_entries() {
        let temp = TempDir::new().unwrap();
        let storage = LedgerStorage::new(temp.path());
        storage.init().unwrap();
        
        // Add some questions
        for i in 0..3 {
            let q = Question::new(
                "banner".to_string(),
                "test".to_string(),
                format!("Question {}", i),
                vec![],
            );
            storage.save_question(&q).unwrap();
        }
        
        let questions = storage.list_questions().unwrap();
        assert_eq!(questions.len(), 3);
    }

    #[test]
    fn test_index_updates() {
        let temp = TempDir::new().unwrap();
        let storage = LedgerStorage::new(temp.path());
        storage.init().unwrap();
        
        let index = storage.load_index().unwrap();
        assert_eq!(index.counts.questions, 0);
        
        let q = Question::new(
            "banner".to_string(),
            "test".to_string(),
            "Question".to_string(),
            vec![],
        );
        storage.save_question(&q).unwrap();
        
        let index = storage.load_index().unwrap();
        assert_eq!(index.counts.questions, 1);
    }
}
