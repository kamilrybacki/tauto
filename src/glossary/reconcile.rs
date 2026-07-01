use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::glossary::models::Glossary;

/// State values observed in real data, keyed by `"Entity.field"`. Produced by a
/// [`StateSource`] adapter (a live database, an ODCS contract, or a native
/// descriptor file) and reconciled against the glossary's declared states.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ObservedDomains {
    pub fields: BTreeMap<String, Vec<String>>,
}

impl ObservedDomains {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record the observed values for `Entity.field`.
    pub fn insert(&mut self, entity: &str, field: &str, values: Vec<String>) {
        self.fields.insert(format!("{entity}.{field}"), values);
    }

    fn get(&self, entity: &str, field: &str) -> Option<&Vec<String>> {
        self.fields.get(&format!("{entity}.{field}"))
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

/// A source of observed state domains. Implementors adapt a data source (live
/// database, ODCS data contract, native file) into [`ObservedDomains`].
pub trait StateSource {
    /// A short label for the source (`"database"`, `"file"`, …) reported to the
    /// caller so it knows where the observed states came from.
    fn label(&self) -> &str;

    /// Read observed state domains. Errors are the source's own (I/O, query);
    /// callers decide whether to fall back.
    fn observed_domains(&self) -> Result<ObservedDomains, String>;
}

/// The reconciliation of one entity state field: its declared domain vs. what a
/// data source observed. Advisory only — it proposes completions, never mutates
/// the glossary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateFieldDiff {
    pub entity: String,
    pub state_field: String,
    pub declared: Vec<String>,
    pub observed: Vec<String>,
    /// Observed in data but not declared — **suggested completions** to add to
    /// the glossary.
    pub observed_not_declared: Vec<String>,
    /// Declared but not observed — a future/unused state, or a typo to review.
    pub declared_not_observed: Vec<String>,
    /// True when the source had no observation for this field at all (so the
    /// `declared_not_observed` list is "no data", not "absent from data").
    pub no_observation: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReconcileReport {
    /// Where the observed domains came from: `"database"`, `"file"`, `"none"`.
    pub source: String,
    pub diffs: Vec<StateFieldDiff>,
}

/// Reconcile declared state domains against observed ones. Only state
/// (determinant) fields are considered; an entity without state fields, or a
/// field the source never observed, still appears (with `no_observation`) so the
/// caller sees full coverage.
pub fn reconcile(glossary: &Glossary, observed: &ObservedDomains, source: &str) -> ReconcileReport {
    let mut diffs = Vec::new();
    for entity in &glossary.entities {
        for state in entity.state_fields() {
            let declared = state.enum_values.clone();
            match observed.get(&entity.name, &state.name) {
                Some(obs) => {
                    let observed_not_declared: Vec<String> =
                        obs.iter().filter(|v| !declared.contains(v)).cloned().collect();
                    let declared_not_observed: Vec<String> =
                        declared.iter().filter(|v| !obs.contains(v)).cloned().collect();
                    diffs.push(StateFieldDiff {
                        entity: entity.name.clone(),
                        state_field: state.name.clone(),
                        declared,
                        observed: obs.clone(),
                        observed_not_declared,
                        declared_not_observed,
                        no_observation: false,
                    });
                }
                None => diffs.push(StateFieldDiff {
                    entity: entity.name.clone(),
                    state_field: state.name.clone(),
                    declared: declared.clone(),
                    observed: Vec::new(),
                    observed_not_declared: Vec::new(),
                    declared_not_observed: declared,
                    no_observation: true,
                }),
            }
        }
    }
    ReconcileReport { source: source.to_owned(), diffs }
}

// ── file fallback: a native JSON descriptor ────────────────────────────────────

/// A file-based [`StateSource`]: reads a native JSON descriptor mapping
/// `"Entity.field": ["Value", …]`. This is the fallback used when no database is
/// configured, and the shape every other adapter normalizes into.
pub struct FileStateSource {
    path: std::path::PathBuf,
}

impl FileStateSource {
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Whether the descriptor file exists (so the caller can decide to use it).
    pub fn exists(&self) -> bool {
        self.path.is_file()
    }
}

impl StateSource for FileStateSource {
    fn label(&self) -> &str {
        "file"
    }

    fn observed_domains(&self) -> Result<ObservedDomains, String> {
        let text = std::fs::read_to_string(&self.path)
            .map_err(|e| format!("reading {}: {e}", self.path.display()))?;
        let fields: BTreeMap<String, Vec<String>> = serde_json::from_str(&text)
            .map_err(|e| format!("parsing {}: {e}", self.path.display()))?;
        Ok(ObservedDomains { fields })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glossary::models::{EntityDef, FieldDef};

    fn glossary() -> Glossary {
        let mut order = EntityDef::new("Order");
        order.fields = vec![FieldDef {
            name: "status".to_owned(),
            type_name: "enum".to_owned(),
            enum_values: vec!["Unpaid".to_owned(), "Paid".to_owned(), "Shipped".to_owned()],
            state: true,
        }];
        Glossary::new(vec![order])
    }

    #[test]
    fn observed_not_declared_becomes_suggested_completion() {
        let mut obs = ObservedDomains::new();
        // Data has a Refunded status the glossary never declared.
        obs.insert("Order", "status", vec![
            "Unpaid".into(), "Paid".into(), "Shipped".into(), "Refunded".into(),
        ]);
        let r = reconcile(&glossary(), &obs, "file");
        assert_eq!(r.diffs.len(), 1);
        assert_eq!(r.diffs[0].observed_not_declared, vec!["Refunded"]);
        assert!(r.diffs[0].declared_not_observed.is_empty());
        assert!(!r.diffs[0].no_observation);
    }

    #[test]
    fn declared_not_observed_reported() {
        let mut obs = ObservedDomains::new();
        obs.insert("Order", "status", vec!["Unpaid".into(), "Paid".into()]);
        let r = reconcile(&glossary(), &obs, "file");
        assert_eq!(r.diffs[0].declared_not_observed, vec!["Shipped"]);
        assert!(r.diffs[0].observed_not_declared.is_empty());
    }

    #[test]
    fn no_observation_flagged_when_source_silent() {
        let obs = ObservedDomains::new(); // nothing observed
        let r = reconcile(&glossary(), &obs, "none");
        assert!(r.diffs[0].no_observation);
        assert_eq!(r.diffs[0].declared_not_observed, r.diffs[0].declared);
    }

    #[test]
    fn perfect_match_has_empty_diffs() {
        let mut obs = ObservedDomains::new();
        obs.insert("Order", "status", vec!["Unpaid".into(), "Paid".into(), "Shipped".into()]);
        let r = reconcile(&glossary(), &obs, "file");
        assert!(r.diffs[0].observed_not_declared.is_empty());
        assert!(r.diffs[0].declared_not_observed.is_empty());
    }

    #[test]
    fn file_source_round_trips_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("_observed_states.json");
        std::fs::write(&path, r#"{"Order.status":["Unpaid","Paid","Shipped","Refunded"]}"#).unwrap();
        let src = FileStateSource::new(&path);
        assert!(src.exists());
        let obs = src.observed_domains().unwrap();
        let r = reconcile(&glossary(), &obs, src.label());
        assert_eq!(r.source, "file");
        assert_eq!(r.diffs[0].observed_not_declared, vec!["Refunded"]);
    }
}
