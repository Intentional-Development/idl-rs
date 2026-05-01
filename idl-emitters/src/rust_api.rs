use crate::traits::{Emitter, EmitResult, GeneratedFile};
use idl_core::IdlDocument;
use anyhow::Result;

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
