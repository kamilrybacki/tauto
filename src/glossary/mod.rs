pub mod db_source;
pub mod lifecycle;
pub mod models;
pub mod parser;
pub mod reconcile;
pub mod validate;

pub use lifecycle::{analyze as analyze_lifecycle, StateCoverage, Transition};
pub use models::{EntityDef, FieldDef, Glossary};
pub use parser::{extract_glossary_blocks, parse_glossary_block};
pub use reconcile::{
    reconcile, FileStateSource, ObservedDomains, ReconcileReport, StateFieldDiff, StateSource,
};
pub use validate::{validate, GlossaryWarning};

/// Extract and parse every ```glossary block in a markdown document into a
/// partial glossary (one document's worth of entities).
pub fn parse_glossary_doc(markdown: &str) -> Vec<EntityDef> {
    extract_glossary_blocks(markdown)
        .iter()
        .filter_map(|b| parse_glossary_block(b))
        .collect()
}
