use crate::traits::Extractor;
use idl_core::IdlDocument;
use anyhow::Result;

pub struct TypeScriptExtractor;

impl Extractor for TypeScriptExtractor {
    fn extract(&self, _source_dir: &str) -> Result<IdlDocument> {
        // TODO: Implement TypeScript extractor
        anyhow::bail!("TypeScript extraction not yet implemented")
    }

    fn source_language(&self) -> &str {
        "typescript"
    }
}
