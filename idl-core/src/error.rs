use std::fmt;
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum IdlError {
    #[error("Parse error at line {line}, column {column}: {message}")]
    ParseError {
        line: usize,
        column: usize,
        message: String,
    },
    
    #[error("Semantic error: {0}")]
    SemanticError(String),
    
    #[error("Version error: {0}")]
    VersionError(String),
    
    #[error("IO error: {0}")]
    IoError(String),
    
    #[error("Unknown block type: {0}")]
    UnknownBlockType(String),
    
    #[error("Missing required field: {0}")]
    MissingRequiredField(String),
}

pub type Result<T> = std::result::Result<T, IdlError>;
