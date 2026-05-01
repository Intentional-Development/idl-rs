use crate::traits::Extractor;
use idl_core::IdlDocument;
use anyhow::Result;

pub struct DartExtractor;

impl Extractor for DartExtractor {
    fn extract(&self, _source_dir: &str) -> Result<IdlDocument> {
        // TODO: Implement Dart extractor
        anyhow::bail!("Dart extraction not yet implemented")
    }

    fn source_language(&self) -> &str {
        "dart"
    }
}
