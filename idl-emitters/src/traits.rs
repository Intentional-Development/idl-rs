use anyhow::Result;
use idl_core::IdlDocument;

/// Emitter trait - transforms IDL AST to target language code
pub trait Emitter {
    fn emit(&self, doc: &IdlDocument) -> Result<EmitResult>;
    fn target_language(&self) -> &str;
}

pub struct EmitResult {
    pub files: Vec<GeneratedFile>,
}

pub struct GeneratedFile {
    pub path: String,
    pub content: String,
}
