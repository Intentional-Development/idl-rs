use crate::traits::Extractor;
use anyhow::Result;
use idl_core::IdlDocument;

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
