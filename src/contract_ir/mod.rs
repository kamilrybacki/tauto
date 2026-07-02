pub mod conflicts;
pub mod diff;
pub mod models;
pub mod serialization;

pub use conflicts::{find_conflict_candidates, ConflictCandidate};
pub use diff::{compare, ContractKey, ContractModification, ContractSetDiff};
pub use models::{
    Condition, ContractIR, ContractSet, Diagnostic, Expression, ExpressionValue,
    ForbiddenOperation, RuleExample, SourceLocation,
};
pub use serialization::{
    canonical_contract_set_json, contract_set_hash, semantic_contract_set_hash,
    semantic_contract_set_json,
};
