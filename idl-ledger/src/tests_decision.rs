#[cfg(test)]
mod tests {
    use crate::{Decision, DecisionScope};
    use uuid::Uuid;

    #[test]
    fn test_decision_creation() {
        let d = Decision::new(
            "stark".to_string(),
            DecisionScope::Tooling,
            "Use Rust for CLI".to_string(),
            "Performance and safety benefits".to_string(),
            None,
            vec![],
        );
        
        assert_eq!(d.decided_by, "stark");
        assert_eq!(d.scope, DecisionScope::Tooling);
        assert_eq!(d.title, "Use Rust for CLI");
    }

    #[test]
    fn test_decision_add_evidence() {
        let mut d = Decision::new(
            "stark".to_string(),
            DecisionScope::Architecture,
            "Use microservices".to_string(),
            "Scalability needs".to_string(),
            None,
            vec![],
        );
        
        let evidence_id = Uuid::new_v4();
        d.add_evidence(evidence_id);
        
        assert_eq!(d.evidence_refs.len(), 1);
        assert!(d.evidence_refs.contains(&evidence_id));
    }

    #[test]
    fn test_decision_supersedes() {
        let old_decision = Uuid::new_v4();
        let d = Decision::new(
            "stark".to_string(),
            DecisionScope::Process,
            "New branching strategy".to_string(),
            "Simpler workflow".to_string(),
            Some(old_decision),
            vec![],
        );
        
        assert_eq!(d.supersedes, Some(old_decision));
    }
}
