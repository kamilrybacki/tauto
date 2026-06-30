pub mod dsl;
pub mod markdown;

pub use dsl::{ParseResult, parse_contract_block};
pub use markdown::{ContractBlock, extract_contract_blocks};
