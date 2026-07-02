use std::collections::HashMap;

use crate::contract_ir::{Condition, ContractIR, ContractSet, ForbiddenOperation};

#[derive(Debug, Clone, PartialEq)]
pub struct ContractKey {
    pub entity: String,
    pub operation: String,
    pub case: String,
}

impl ContractKey {
    pub fn from_contract(c: &ContractIR) -> Self {
        Self { entity: c.entity.clone(), operation: c.operation.clone(), case: c.case.clone() }
    }

    pub fn to_display(&self) -> String {
        format!("{}/{}/{}", self.entity, self.operation, self.case)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContractModification {
    pub key: ContractKey,
    pub requires_added: Vec<Condition>,
    pub requires_removed: Vec<Condition>,
    pub ensures_added: Vec<Condition>,
    pub ensures_removed: Vec<Condition>,
    pub forbidden_added: Vec<ForbiddenOperation>,
    pub forbidden_removed: Vec<ForbiddenOperation>,
    pub preserves_added: Vec<String>,
    pub preserves_removed: Vec<String>,
    pub assumes_added: Vec<String>,
    pub assumes_removed: Vec<String>,
}

impl ContractModification {
    pub fn has_removals(&self) -> bool {
        !self.requires_removed.is_empty()
            || !self.ensures_removed.is_empty()
            || !self.forbidden_removed.is_empty()
            || !self.preserves_removed.is_empty()
            || !self.assumes_removed.is_empty()
    }

    pub fn has_additions(&self) -> bool {
        !self.requires_added.is_empty()
            || !self.ensures_added.is_empty()
            || !self.forbidden_added.is_empty()
            || !self.preserves_added.is_empty()
            || !self.assumes_added.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContractSetDiff {
    pub added: Vec<ContractIR>,
    pub removed: Vec<ContractIR>,
    pub modified: Vec<ContractModification>,
    /// No contracts removed and no conditions removed within modified contracts.
    pub is_expansion_only: bool,
}

impl ContractSetDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty()
    }
}

pub fn compare(base: &ContractSet, new: &ContractSet) -> ContractSetDiff {
    let base_map: HashMap<String, &ContractIR> =
        base.contracts.iter().map(|c| (key_str(c), c)).collect();
    let new_map: HashMap<String, &ContractIR> =
        new.contracts.iter().map(|c| (key_str(c), c)).collect();

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut modified = Vec::new();

    for (k, new_c) in &new_map {
        if !base_map.contains_key(k.as_str()) {
            added.push((*new_c).clone());
        }
    }
    added.sort_by_key(key_str);

    for (k, base_c) in &base_map {
        match new_map.get(k.as_str()) {
            None => removed.push((*base_c).clone()),
            Some(new_c) => {
                let m = diff_contract(base_c, new_c);
                if m.has_additions() || m.has_removals() {
                    modified.push(m);
                }
            }
        }
    }
    removed.sort_by_key(key_str);
    modified.sort_by_key(|m| m.key.to_display());

    let is_expansion_only =
        removed.is_empty() && !modified.iter().any(|m| m.has_removals());

    ContractSetDiff { added, removed, modified, is_expansion_only }
}

fn key_str(c: &ContractIR) -> String {
    format!("{}::{}::{}", c.entity, c.operation, c.case)
}

fn diff_contract(base: &ContractIR, new: &ContractIR) -> ContractModification {
    ContractModification {
        key: ContractKey::from_contract(new),
        requires_added: vec_added(&base.requires, &new.requires),
        requires_removed: vec_removed(&base.requires, &new.requires),
        ensures_added: vec_added(&base.ensures, &new.ensures),
        ensures_removed: vec_removed(&base.ensures, &new.ensures),
        forbidden_added: vec_added(&base.forbidden, &new.forbidden),
        forbidden_removed: vec_removed(&base.forbidden, &new.forbidden),
        preserves_added: vec_added(&base.preserves, &new.preserves),
        preserves_removed: vec_removed(&base.preserves, &new.preserves),
        assumes_added: vec_added(&base.assumes, &new.assumes),
        assumes_removed: vec_removed(&base.assumes, &new.assumes),
    }
}

fn vec_added<T: PartialEq + Clone>(base: &[T], new: &[T]) -> Vec<T> {
    new.iter().filter(|item| !base.contains(item)).cloned().collect()
}

fn vec_removed<T: PartialEq + Clone>(base: &[T], new: &[T]) -> Vec<T> {
    base.iter().filter(|item| !new.contains(item)).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract_ir::{Condition, ContractIR, ContractSet, Expression, ExpressionValue};

    fn field(v: &str) -> Expression {
        Expression { kind: "field".to_owned(), value: ExpressionValue::Str(v.to_owned()) }
    }
    fn enom(v: &str) -> Expression {
        Expression { kind: "enum".to_owned(), value: ExpressionValue::Str(v.to_owned()) }
    }
    fn cond(left: &str, op: &str, right: &str) -> Condition {
        Condition { left: field(left), operator: op.to_owned(), right: enom(right) }
    }

    fn cancel_base() -> ContractIR {
        ContractIR {
            case: "CancelPaidOrder".to_owned(),
            entity: "Order".to_owned(),
            operation: "cancelOrder".to_owned(),
            requires: vec![cond("order.status", "==", "Paid")],
            ensures: vec![cond("result.status", "==", "Cancelled")],
            forbidden: vec![],
            preserves: vec![],
            assumes: vec![],
            intent: None, examples: Vec::new(), source: None,
        }
    }

    #[test]
    fn identical_sets_produce_empty_diff() {
        let cs = ContractSet::new(vec![cancel_base()]);
        let diff = compare(&cs, &cs);
        assert!(diff.is_empty());
        assert!(diff.is_expansion_only);
    }

    #[test]
    fn new_contract_appears_as_added() {
        let base = ContractSet::new(vec![cancel_base()]);
        let new = ContractSet::new(vec![
            cancel_base(),
            ContractIR::new("ShipOrder", "Order", "shipOrder"),
        ]);
        let diff = compare(&base, &new);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].case, "ShipOrder");
        assert!(diff.removed.is_empty());
        assert!(diff.modified.is_empty());
    }

    #[test]
    fn removed_contract_appears_as_removed() {
        let base = ContractSet::new(vec![
            cancel_base(),
            ContractIR::new("ShipOrder", "Order", "shipOrder"),
        ]);
        let new = ContractSet::new(vec![cancel_base()]);
        let diff = compare(&base, &new);
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.removed[0].case, "ShipOrder");
        assert!(diff.added.is_empty());
        assert!(diff.modified.is_empty());
    }

    #[test]
    fn condition_added_to_requires_shows_in_modification() {
        let base = ContractSet::new(vec![cancel_base()]);
        let mut updated = cancel_base();
        updated.requires.push(cond("order.refundable", "==", "true"));
        let new = ContractSet::new(vec![updated]);
        let diff = compare(&base, &new);
        assert_eq!(diff.modified.len(), 1);
        let m = &diff.modified[0];
        assert_eq!(m.requires_added.len(), 1);
        assert!(m.requires_removed.is_empty());
    }

    #[test]
    fn condition_removed_from_requires_shows_in_modification() {
        let base = ContractSet::new(vec![cancel_base()]);
        let mut lighter = cancel_base();
        lighter.requires.clear();
        let new = ContractSet::new(vec![lighter]);
        let diff = compare(&base, &new);
        assert_eq!(diff.modified.len(), 1);
        let m = &diff.modified[0];
        assert_eq!(m.requires_removed.len(), 1);
        assert!(m.requires_added.is_empty());
    }

    #[test]
    fn is_expansion_only_true_when_only_contracts_added() {
        let base = ContractSet::new(vec![cancel_base()]);
        let new = ContractSet::new(vec![
            cancel_base(),
            ContractIR::new("ShipOrder", "Order", "shipOrder"),
        ]);
        let diff = compare(&base, &new);
        assert!(diff.is_expansion_only);
    }

    #[test]
    fn is_expansion_only_false_when_contract_removed() {
        let base = ContractSet::new(vec![
            cancel_base(),
            ContractIR::new("ShipOrder", "Order", "shipOrder"),
        ]);
        let new = ContractSet::new(vec![cancel_base()]);
        let diff = compare(&base, &new);
        assert!(!diff.is_expansion_only);
    }

    #[test]
    fn is_expansion_only_false_when_condition_removed() {
        let base = ContractSet::new(vec![cancel_base()]);
        let mut lighter = cancel_base();
        lighter.requires.clear();
        let new = ContractSet::new(vec![lighter]);
        let diff = compare(&base, &new);
        assert!(!diff.is_expansion_only);
    }

    #[test]
    fn is_expansion_only_true_when_condition_added_within_modified() {
        let base = ContractSet::new(vec![cancel_base()]);
        let mut tighter = cancel_base();
        tighter.requires.push(cond("order.refundable", "==", "true"));
        let new = ContractSet::new(vec![tighter]);
        let diff = compare(&base, &new);
        assert!(diff.is_expansion_only);
    }

    #[test]
    fn unchanged_contract_not_in_modified_list() {
        let cs = ContractSet::new(vec![cancel_base(), ContractIR::new("X", "E", "op")]);
        let diff = compare(&cs, &cs);
        assert!(diff.modified.is_empty());
    }

    #[test]
    fn contract_key_display_format() {
        let key = ContractKey::from_contract(&cancel_base());
        assert_eq!(key.to_display(), "Order/cancelOrder/CancelPaidOrder");
    }
}
