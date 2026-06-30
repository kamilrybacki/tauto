use sha2::{Digest, Sha256};

use super::models::{ContractIR, ContractSet};

fn semantic_contract_value(contract: &ContractIR) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert("case".to_owned(), serde_json::Value::String(contract.case.clone()));
    map.insert("entity".to_owned(), serde_json::Value::String(contract.entity.clone()));
    map.insert("operation".to_owned(), serde_json::Value::String(contract.operation.clone()));
    if !contract.requires.is_empty() {
        map.insert("requires".to_owned(), serde_json::to_value(&contract.requires).unwrap());
    }
    if !contract.ensures.is_empty() {
        map.insert("ensures".to_owned(), serde_json::to_value(&contract.ensures).unwrap());
    }
    if !contract.forbidden.is_empty() {
        map.insert("forbidden".to_owned(), serde_json::to_value(&contract.forbidden).unwrap());
    }
    if !contract.preserves.is_empty() {
        map.insert("preserves".to_owned(), serde_json::to_value(&contract.preserves).unwrap());
    }
    if !contract.assumes.is_empty() {
        map.insert("assumes".to_owned(), serde_json::to_value(&contract.assumes).unwrap());
    }
    // source intentionally excluded
    serde_json::Value::Object(map)
}

pub fn semantic_contract_set_json(contract_set: &ContractSet) -> String {
    let contracts: Vec<serde_json::Value> =
        contract_set.contracts.iter().map(semantic_contract_value).collect();
    let payload = serde_json::json!({
        "schema_version": contract_set.schema_version,
        "contracts": contracts,
    });
    serde_json::to_string(&payload).unwrap()
}

pub fn semantic_contract_set_hash(contract_set: &ContractSet) -> String {
    let json = semantic_contract_set_json(contract_set);
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn canonical_contract_set_json(contract_set: &ContractSet) -> String {
    serde_json::to_string(contract_set).unwrap()
}

pub fn contract_set_hash(contract_set: &ContractSet) -> String {
    let json = canonical_contract_set_json(contract_set);
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract_ir::models::{ContractIR, ContractSet, SourceLocation};

    fn simple_set() -> ContractSet {
        ContractSet::new(vec![ContractIR::new("CancelPaidOrder", "Order", "cancelOrder")])
    }

    fn set_with_source() -> ContractSet {
        let mut c = ContractIR::new("CancelPaidOrder", "Order", "cancelOrder");
        c.source = Some(SourceLocation {
            document_path: "spec.md".to_owned(),
            start_line: 10,
            end_line: 20,
        });
        ContractSet::new(vec![c])
    }

    #[test]
    fn semantic_hash_is_64_hex_chars() {
        let h = semantic_contract_set_hash(&simple_set());
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn semantic_hash_excludes_source_location() {
        let h1 = semantic_contract_set_hash(&simple_set());
        let h2 = semantic_contract_set_hash(&set_with_source());
        assert_eq!(h1, h2, "adding source should not change semantic hash");
    }

    #[test]
    fn semantic_hash_changes_when_case_changes() {
        let mut cs = simple_set();
        cs.contracts[0].case = "ModifiedCase".to_owned();
        let h2 = semantic_contract_set_hash(&cs);
        assert_ne!(semantic_contract_set_hash(&simple_set()), h2);
    }

    #[test]
    fn full_hash_differs_when_source_added() {
        let h1 = contract_set_hash(&simple_set());
        let h2 = contract_set_hash(&set_with_source());
        assert_ne!(h1, h2, "provenance hash must include source");
    }

    #[test]
    fn semantic_hash_is_stable_across_calls() {
        let h1 = semantic_contract_set_hash(&simple_set());
        let h2 = semantic_contract_set_hash(&simple_set());
        assert_eq!(h1, h2);
    }

    #[test]
    fn semantic_json_does_not_contain_source_key() {
        let json = semantic_contract_set_json(&set_with_source());
        assert!(!json.contains("source"), "source must be excluded from semantic JSON");
    }
}
