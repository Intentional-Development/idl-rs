use crate::traits::Extractor;
use idl_core::IdlDocument;
use anyhow::Result;

pub struct JavaScriptExtractor;

impl Extractor for JavaScriptExtractor {
    fn extract(&self, _source_dir: &str) -> Result<IdlDocument> {
        // TODO: Implement JavaScript extractor
        anyhow::bail!("JavaScript extraction not yet implemented")
    }

    fn source_language(&self) -> &str {
        "javascript"
    }
}
