use crate::traits::{EmitResult, Emitter, GeneratedFile};
use anyhow::Result;
use idl_core::IdlDocument;

pub struct PythonApiEmitter;

impl Emitter for PythonApiEmitter {
    fn emit(&self, _doc: &IdlDocument) -> Result<EmitResult> {
        // TODO: Implement Python API emitter
        Ok(EmitResult {
            files: vec![GeneratedFile {
                path: "api.py".to_string(),
                content: "# Python API - not yet implemented\n".to_string(),
            }],
        })
    }

    fn target_language(&self) -> &str {
        "python"
    }
}
