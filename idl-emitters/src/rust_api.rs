use crate::traits::{EmitResult, Emitter, GeneratedFile};
use anyhow::Result;
use idl_core::IdlDocument;

pub struct RustApiEmitter;

impl Emitter for RustApiEmitter {
    fn emit(&self, _doc: &IdlDocument) -> Result<EmitResult> {
        // TODO: Implement Rust API emitter
        Ok(EmitResult {
            files: vec![GeneratedFile {
                path: "api.rs".to_string(),
                content: "// Rust API - not yet implemented\n".to_string(),
            }],
        })
    }

    fn target_language(&self) -> &str {
        "rust"
    }
}
