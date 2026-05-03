use crate::traits::{EmitResult, Emitter, GeneratedFile};
use anyhow::Result;
use idl_core::IdlDocument;

pub struct NodeApiEmitter;

impl Emitter for NodeApiEmitter {
    fn emit(&self, _doc: &IdlDocument) -> Result<EmitResult> {
        // TODO: Implement Node.js API emitter
        Ok(EmitResult {
            files: vec![GeneratedFile {
                path: "api.ts".to_string(),
                content: "// Node.js API - not yet implemented\n".to_string(),
            }],
        })
    }

    fn target_language(&self) -> &str {
        "node"
    }
}
