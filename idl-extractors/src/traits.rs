use anyhow::Result;
use idl_core::IdlDocument;

/// Extractor trait - analyzes brownfield code and generates IDL
pub trait Extractor {
    fn extract(&self, source_dir: &str) -> Result<IdlDocument>;
    fn source_language(&self) -> &str;
}
