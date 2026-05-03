#[cfg(test)]
mod tests {
    use crate::{Question, QuestionStatus};
    use uuid::Uuid;

    #[test]
    fn test_question_lifecycle() {
        let q = Question::new(
            "banner".to_string(),
            "architecture".to_string(),
            "Should we use Rust or Go?".to_string(),
            vec![],
        );
        
        assert_eq!(q.asked_by, "banner");
        assert_eq!(q.status, QuestionStatus::Open);
        assert!(q.answered_by_decision.is_none());
    }

    #[test]
    fn test_question_answer() {
        let mut q = Question::new(
            "banner".to_string(),
            "architecture".to_string(),
            "Should we use Rust or Go?".to_string(),
            vec![],
        );
        
        let decision_id = Uuid::new_v4();
        q.answer(decision_id);
        
        assert_eq!(q.status, QuestionStatus::Answered);
        assert_eq!(q.answered_by_decision, Some(decision_id));
    }

    #[test]
    fn test_question_abandon() {
        let mut q = Question::new(
            "banner".to_string(),
            "architecture".to_string(),
            "Should we use Rust or Go?".to_string(),
            vec![],
        );
        
        q.abandon();
        assert_eq!(q.status, QuestionStatus::Abandoned);
    }
}
