use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use crate::contract_ir::{ContractSet, semantic_contract_set_hash};

#[derive(Debug, Clone, PartialEq)]
pub struct DeterministicContext {
    pub entries: BTreeMap<String, String>,
    pub context_hash: String,
}

pub fn build_deterministic_context(
    contract_set: &ContractSet,
    generator_intent: &str,
) -> DeterministicContext {
    let mut entries = BTreeMap::new();
    entries.insert("contract_count".to_owned(), contract_set.contracts.len().to_string());
    entries.insert("contract_set_hash".to_owned(), semantic_contract_set_hash(contract_set));
    entries.insert("generator_intent".to_owned(), generator_intent.to_owned());

    let json = serde_json::to_string(&entries).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    let context_hash = format!("{:x}", hasher.finalize());

    DeterministicContext { entries, context_hash }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract_ir::{ContractIR, ContractSet, SourceLocation};

    fn one_contract() -> ContractSet {
        ContractSet::new(vec![ContractIR::new("CancelPaidOrder", "Order", "cancelOrder")])
    }

    #[test]
    fn context_hash_is_64_hex_chars() {
        let ctx = build_deterministic_context(&one_contract(), "lean_gen");
        assert_eq!(ctx.context_hash.len(), 64);
    }

    #[test]
    fn context_contains_expected_keys() {
        let ctx = build_deterministic_context(&one_contract(), "lean_gen");
        assert!(ctx.entries.contains_key("contract_count"));
        assert!(ctx.entries.contains_key("contract_set_hash"));
        assert!(ctx.entries.contains_key("generator_intent"));
    }

    #[test]
    fn context_contract_count_matches() {
        let cs = ContractSet::new(vec![
            ContractIR::new("A", "E", "op"),
            ContractIR::new("B", "E", "op"),
        ]);
        let ctx = build_deterministic_context(&cs, "lean_gen");
        assert_eq!(ctx.entries["contract_count"], "2");
    }

    #[test]
    fn context_hash_is_stable() {
        let ctx1 = build_deterministic_context(&one_contract(), "lean_gen");
        let ctx2 = build_deterministic_context(&one_contract(), "lean_gen");
        assert_eq!(ctx1.context_hash, ctx2.context_hash);
    }

    #[test]
    fn context_hash_changes_with_different_intent() {
        let ctx1 = build_deterministic_context(&one_contract(), "lean_gen");
        let ctx2 = build_deterministic_context(&one_contract(), "coq_gen");
        assert_ne!(ctx1.context_hash, ctx2.context_hash);
    }

    #[test]
    fn source_location_change_does_not_affect_context_hash() {
        let cs1 = one_contract();
        let mut c2 = ContractIR::new("CancelPaidOrder", "Order", "cancelOrder");
        c2.source = Some(SourceLocation { document_path: "spec.md".to_owned(), start_line: 1, end_line: 1 });
        let cs2 = ContractSet::new(vec![c2]);
        let ctx1 = build_deterministic_context(&cs1, "lean_gen");
        let ctx2 = build_deterministic_context(&cs2, "lean_gen");
        assert_eq!(ctx1.context_hash, ctx2.context_hash);
    }

    #[test]
    fn entries_are_sorted_deterministically() {
        let ctx = build_deterministic_context(&one_contract(), "lean_gen");
        let keys: Vec<&str> = ctx.entries.keys().map(|s| s.as_str()).collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted, "BTreeMap must maintain sorted order");
    }
}
