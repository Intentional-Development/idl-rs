use crate::traits::Extractor;
use idl_core::IdlDocument;
use anyhow::Result;

pub struct PhpExtractor;

impl Extractor for PhpExtractor {
    fn extract(&self, _source_dir: &str) -> Result<IdlDocument> {
        // TODO: Implement PHP extractor
        anyhow::bail!("PHP extraction not yet implemented")
    }

    fn source_language(&self) -> &str {
        "php"
    }
}
