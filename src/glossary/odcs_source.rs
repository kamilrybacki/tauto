//! ODCS ([Open Data Contract Standard](https://bitol-io.github.io/open-data-contract-standard/))
//! [`StateSource`](crate::glossary::StateSource) for state reconciliation.
//!
//! An ODCS data contract describes a data product's schema: `schema[]` objects
//! (tables) with `properties[]` (columns). A column's allowed values are
//! expressed as a data-quality rule — the `invalidValues` metric with an
//! `arguments.validValues` list. This adapter reads those lists and maps each
//! (schema, property) onto a glossary entity state field, so declared states can
//! be reconciled against a data contract. Pure and offline — no database needed.

use serde::Deserialize;

use crate::glossary::models::Glossary;
use crate::glossary::reconcile::{ObservedDomains, StateSource};

// ── minimal ODCS shape (only the fields we read) ───────────────────────────────

#[derive(Debug, Deserialize)]
struct OdcsDoc {
    #[serde(default)]
    schema: Vec<OdcsSchema>,
}

#[derive(Debug, Deserialize)]
struct OdcsSchema {
    #[serde(default)]
    name: Option<String>,
    #[serde(default, rename = "physicalName")]
    physical_name: Option<String>,
    #[serde(default)]
    properties: Vec<OdcsProperty>,
}

#[derive(Debug, Deserialize)]
struct OdcsProperty {
    #[serde(default)]
    name: Option<String>,
    #[serde(default, rename = "physicalName")]
    physical_name: Option<String>,
    #[serde(default)]
    quality: Vec<OdcsQuality>,
}

#[derive(Debug, Deserialize)]
struct OdcsQuality {
    #[serde(default)]
    arguments: Option<OdcsArgs>,
}

#[derive(Debug, Deserialize)]
struct OdcsArgs {
    #[serde(default, rename = "validValues")]
    valid_values: Vec<serde_yaml::Value>,
}

// ── extraction ─────────────────────────────────────────────────────────────────

fn case_eq(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Resolve an ODCS (schema, property) pair to a glossary entity **state field**,
/// matching case-insensitively on either the logical `name` or `physicalName`.
/// Returns the canonical `(entity, field)` names, or `None` when there is no
/// matching state field.
fn resolve<'g>(
    glossary: &'g Glossary,
    schema_names: &[&str],
    prop_names: &[&str],
) -> Option<(&'g str, &'g str)> {
    for entity in &glossary.entities {
        let entity_match = schema_names.iter().any(|s| {
            case_eq(&entity.name, s) || entity.aka.iter().any(|a| case_eq(a, s))
        });
        if !entity_match {
            continue;
        }
        for field in entity.state_fields() {
            if prop_names.iter().any(|p| case_eq(&field.name, p)) {
                return Some((&entity.name, &field.name));
            }
        }
    }
    None
}

/// Parse a single ODCS YAML document and collect observed state domains,
/// resolved against the glossary's state fields.
pub fn observed_from_odcs_yaml(
    yaml: &str,
    glossary: &Glossary,
) -> Result<ObservedDomains, String> {
    let doc: OdcsDoc =
        serde_yaml::from_str(yaml).map_err(|e| format!("parsing ODCS YAML: {e}"))?;
    let mut observed = ObservedDomains::new();

    for schema in &doc.schema {
        let schema_names: Vec<&str> = [schema.name.as_deref(), schema.physical_name.as_deref()]
            .into_iter()
            .flatten()
            .collect();
        if schema_names.is_empty() {
            continue;
        }
        for prop in &schema.properties {
            let values = collect_valid_values(prop);
            if values.is_empty() {
                continue;
            }
            let prop_names: Vec<&str> = [prop.name.as_deref(), prop.physical_name.as_deref()]
                .into_iter()
                .flatten()
                .collect();
            if let Some((entity, field)) = resolve(glossary, &schema_names, &prop_names) {
                observed.insert(entity, field, values);
            }
        }
    }
    Ok(observed)
}

/// The string `validValues` across all of a property's quality rules, de-duped
/// in first-seen order.
fn collect_valid_values(prop: &OdcsProperty) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for q in &prop.quality {
        let Some(args) = &q.arguments else { continue };
        for v in &args.valid_values {
            if let serde_yaml::Value::String(s) = v {
                if !out.iter().any(|x| x == s) {
                    out.push(s.clone());
                }
            }
        }
    }
    out
}

// ── file-scanning source ───────────────────────────────────────────────────────

/// A [`StateSource`] that reads every `*.odcs.yaml` / `*.odcs.yml` in a directory
/// and merges the observed state domains it can resolve against the glossary.
pub struct OdcsStateSource {
    dir: std::path::PathBuf,
    glossary: Glossary,
}

impl OdcsStateSource {
    pub fn new(dir: impl Into<std::path::PathBuf>, glossary: Glossary) -> Self {
        Self { dir: dir.into(), glossary }
    }

    fn contract_files(&self) -> Vec<std::path::PathBuf> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut files: Vec<std::path::PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                name.ends_with(".odcs.yaml") || name.ends_with(".odcs.yml")
            })
            .collect();
        files.sort();
        files
    }

    /// Whether any ODCS contract file is present.
    pub fn exists(&self) -> bool {
        !self.contract_files().is_empty()
    }
}

impl StateSource for OdcsStateSource {
    fn label(&self) -> &str {
        "odcs"
    }

    fn observed_domains(&self) -> Result<ObservedDomains, String> {
        let mut merged = ObservedDomains::new();
        for path in self.contract_files() {
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("reading {}: {e}", path.display()))?;
            let obs = observed_from_odcs_yaml(&text, &self.glossary)?;
            for (k, v) in obs.fields {
                merged.fields.entry(k).or_insert(v);
            }
        }
        Ok(merged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glossary::models::{EntityDef, FieldDef};

    const CONTRACT: &str = r#"
schema:
  - name: Order
    physicalName: orders
    properties:
      - name: status
        physicalName: order_status
        quality:
          - metric: invalidValues
            arguments:
              validValues: ['Unpaid', 'Paid', 'Shipped', 'Refunded']
      - name: amount
        logicalType: number
"#;

    fn order_glossary() -> Glossary {
        let mut order = EntityDef::new("Order");
        order.aka = vec!["order".to_owned()];
        order.fields = vec![
            FieldDef {
                name: "status".to_owned(),
                type_name: "enum".to_owned(),
                enum_values: vec!["Unpaid".to_owned(), "Paid".to_owned(), "Shipped".to_owned()],
                state: true,
            },
            FieldDef::new("amount", "int"),
        ];
        Glossary::new(vec![order])
    }

    #[test]
    fn extracts_valid_values_for_state_field() {
        let obs = observed_from_odcs_yaml(CONTRACT, &order_glossary()).unwrap();
        assert_eq!(
            obs.fields.get("Order.status").unwrap(),
            &vec!["Unpaid", "Paid", "Shipped", "Refunded"]
        );
    }

    #[test]
    fn non_state_property_is_ignored() {
        let obs = observed_from_odcs_yaml(CONTRACT, &order_glossary()).unwrap();
        assert!(!obs.fields.contains_key("Order.amount"));
    }

    #[test]
    fn matches_by_physical_name_too() {
        // Glossary field named after the physical column.
        let mut order = EntityDef::new("Order");
        order.fields = vec![FieldDef {
            name: "order_status".to_owned(),
            type_name: "enum".to_owned(),
            enum_values: vec!["Paid".to_owned()],
            state: true,
        }];
        let g = Glossary::new(vec![order]);
        let obs = observed_from_odcs_yaml(CONTRACT, &g).unwrap();
        assert!(obs.fields.contains_key("Order.order_status"));
    }

    #[test]
    fn unmatched_entity_yields_nothing() {
        let mut other = EntityDef::new("Invoice");
        other.fields = vec![FieldDef {
            name: "status".to_owned(),
            type_name: "enum".to_owned(),
            enum_values: vec![],
            state: true,
        }];
        let obs = observed_from_odcs_yaml(CONTRACT, &Glossary::new(vec![other])).unwrap();
        assert!(obs.is_empty());
    }

    #[test]
    fn reconciles_to_suggested_completion() {
        let obs = observed_from_odcs_yaml(CONTRACT, &order_glossary()).unwrap();
        let report = crate::glossary::reconcile::reconcile(&order_glossary(), &obs, "odcs");
        let d = &report.diffs[0];
        assert_eq!(d.observed_not_declared, vec!["Refunded"]);
    }

    #[test]
    fn malformed_yaml_errors() {
        assert!(observed_from_odcs_yaml("schema: [: :", &order_glossary()).is_err());
    }

    #[test]
    fn file_source_scans_and_merges() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("orders.odcs.yaml"), CONTRACT).unwrap();
        let src = OdcsStateSource::new(dir.path(), order_glossary());
        assert!(src.exists());
        assert_eq!(src.label(), "odcs");
        let obs = src.observed_domains().unwrap();
        assert!(obs.fields.contains_key("Order.status"));
    }

    #[test]
    fn file_source_absent_when_no_contract_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("readme.md"), "# not a contract").unwrap();
        let src = OdcsStateSource::new(dir.path(), order_glossary());
        assert!(!src.exists());
    }
}
