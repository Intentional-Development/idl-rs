use crate::traits::{EmitResult, Emitter, GeneratedFile};
use anyhow::Result;
use idl_core::IdlDocument;

pub struct GoApiEmitter;

impl Emitter for GoApiEmitter {
    fn emit(&self, _doc: &IdlDocument) -> Result<EmitResult> {
        // TODO: Implement Go API emitter
        Ok(EmitResult {
            files: vec![GeneratedFile {
                path: "api.go".to_string(),
                content: "// Go API - not yet implemented\n".to_string(),
            }],
        })
    }

    fn target_language(&self) -> &str {
        "go"
    }
}
