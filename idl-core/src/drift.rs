// Drift detection engine (stub for Wave 7)

use crate::ast::*;
use crate::error::Result;

pub struct DriftReport {
    pub spec_path: String,
    pub code_path: String,
    pub drifts: Vec<Drift>,
}

pub struct Drift {
    pub drift_type: DriftType,
    pub severity: Severity,
    pub message: String,
    pub spec_location: Option<Location>,
    pub code_location: Option<Location>,
}

pub enum DriftType {
    MissingInCode,
    MissingInSpec,
    TypeMismatch,
    SignatureMismatch,
}

pub enum Severity {
    Error,
    Warning,
    Info,
}

pub struct Location {
    pub file: String,
    pub line: usize,
    pub column: usize,
}

impl DriftReport {
    pub fn compare(_spec_doc: &IdlDocument, _code_analysis: &()) -> Result<Self> {
        // TODO: Implement drift detection
        Ok(Self {
            spec_path: String::new(),
            code_path: String::new(),
            drifts: Vec::new(),
        })
    }
}
