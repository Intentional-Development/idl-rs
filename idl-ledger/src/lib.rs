use std::path::PathBuf;

pub mod question;
pub mod decision;
pub mod evidence;
pub mod index;
pub mod storage;
pub mod validate;

pub use question::{Question, QuestionStatus};
pub use decision::{Decision, DecisionScope};
pub use evidence::{Evidence, EvidenceKind};
pub use index::LedgerIndex;
pub use storage::LedgerStorage;
pub use validate::LedgerValidator;

#[derive(Debug, thiserror::Error)]
pub enum LedgerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("WalkDir error: {0}")]
    WalkDir(#[from] walkdir::Error),
    
    #[error("Entry not found: {0}")]
    NotFound(String),
    
    #[error("Invalid reference: {0}")]
    InvalidReference(String),
    
    #[error("Validation error: {0}")]
    ValidationError(String),
}

pub type Result<T> = std::result::Result<T, LedgerError>;

/// Get the ledger directory from environment or use default
pub fn get_ledger_dir() -> PathBuf {
    std::env::var("IDL_LEDGER_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".idl/ledger"))
}

#[cfg(test)]
mod tests_question;
#[cfg(test)]
mod tests_decision;
#[cfg(test)]
mod tests_storage;
#[cfg(test)]
mod tests_validate;
