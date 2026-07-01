//! Live-database [`StateSource`](crate::glossary::StateSource) for state
//! reconciliation. Reads the distinct values of each entity's state column from a
//! Postgres instance (`DATABASE_URL`). Gated behind the `database` feature so the
//! default build carries no database dependency.
//!
//! The identifier resolution, SQL construction, and row folding are pure and
//! always compiled (and unit-tested); only the connection itself is feature-gated
//! and requires a reachable database to exercise.

use crate::glossary::models::Glossary;
#[allow(unused_imports)]
use crate::glossary::reconcile::ObservedDomains;

/// One resolved target: an entity state field mapped to a physical table/column.
#[derive(Debug, Clone, PartialEq)]
pub struct DbTarget {
    pub entity: String,
    pub field: String,
    pub table: String,
    pub column: String,
}

/// A SQL identifier is safe to interpolate (quoted) only if it is a plain
/// `[A-Za-z_][A-Za-z0-9_]*`. Anything else is rejected rather than escaped, so
/// no attacker-influenced string can break out of the quoting.
pub fn valid_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Convention: entity `Mortgage` → table `mortgage`. (Explicit per-entity table
/// overrides are a later refinement.)
pub fn resolve_table(entity_name: &str) -> String {
    entity_name.to_lowercase()
}

/// Convention: a state field's column is its own name (`status` → `status`).
pub fn resolve_column(field_name: &str) -> String {
    field_name.to_owned()
}

/// The `SELECT DISTINCT` query for a table/column, with both identifiers quoted.
/// Returns `None` when either identifier is unsafe (the caller skips that field).
pub fn distinct_query(table: &str, column: &str) -> Option<String> {
    if !valid_ident(table) || !valid_ident(column) {
        return None;
    }
    Some(format!(
        "SELECT DISTINCT \"{column}\" AS v FROM \"{table}\" WHERE \"{column}\" IS NOT NULL"
    ))
}

/// Resolve every state field in the glossary to a `(table, column)` target,
/// skipping fields whose resolved identifiers are unsafe.
pub fn targets(glossary: &Glossary) -> Vec<DbTarget> {
    let mut out = Vec::new();
    for entity in &glossary.entities {
        for state in entity.state_fields() {
            let table = resolve_table(&entity.name);
            let column = resolve_column(&state.name);
            if valid_ident(&table) && valid_ident(&column) {
                out.push(DbTarget {
                    entity: entity.name.clone(),
                    field: state.name.clone(),
                    table,
                    column,
                });
            }
        }
    }
    out
}

/// Attempt to read observed state domains from a live database.
///
/// Returns `None` when the database source is unavailable (feature off, or
/// `DATABASE_URL` unset) so the caller falls back to the file source. Returns
/// `Some(Err(_))` when a database was configured but the read failed.
#[cfg(feature = "database")]
pub fn observed_from_database(glossary: &Glossary) -> Option<Result<ObservedDomains, String>> {
    let url = std::env::var("DATABASE_URL").ok()?;
    Some(read_database(&url, glossary))
}

#[cfg(not(feature = "database"))]
pub fn observed_from_database(_glossary: &Glossary) -> Option<Result<ObservedDomains, String>> {
    None
}

#[cfg(feature = "database")]
fn read_database(url: &str, glossary: &Glossary) -> Result<ObservedDomains, String> {
    use postgres::{Client, NoTls};

    let mut client =
        Client::connect(url, NoTls).map_err(|e| format!("connecting to database: {e}"))?;

    let mut observed = ObservedDomains::new();
    for t in targets(glossary) {
        let Some(sql) = distinct_query(&t.table, &t.column) else {
            continue;
        };
        // A missing table / column should not abort the whole reconciliation;
        // skip that target and carry on.
        let rows = match client.query(sql.as_str(), &[]) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[reconcile:db] {}.{}: {e}", t.table, t.column);
                continue;
            }
        };
        let mut values: Vec<String> = rows
            .iter()
            .filter_map(|row| row.try_get::<_, String>("v").ok())
            .collect();
        values.sort();
        values.dedup();
        observed.insert(&t.entity, &t.field, values);
    }
    Ok(observed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glossary::models::{EntityDef, FieldDef};

    fn glossary_with_state() -> Glossary {
        let mut order = EntityDef::new("Order");
        order.fields = vec![
            FieldDef {
                name: "status".to_owned(),
                type_name: "enum".to_owned(),
                enum_values: vec!["Paid".to_owned()],
                state: true,
            },
            FieldDef::new("amount", "int"),
        ];
        Glossary::new(vec![order])
    }

    #[test]
    fn valid_ident_accepts_plain_identifiers() {
        assert!(valid_ident("status"));
        assert!(valid_ident("order_status"));
        assert!(valid_ident("_x"));
    }

    #[test]
    fn valid_ident_rejects_injection_attempts() {
        assert!(!valid_ident("status; DROP TABLE users"));
        assert!(!valid_ident("a b"));
        assert!(!valid_ident("\"quoted\""));
        assert!(!valid_ident("1col"));
        assert!(!valid_ident(""));
    }

    #[test]
    fn resolve_table_lowercases_entity() {
        assert_eq!(resolve_table("Mortgage"), "mortgage");
    }

    #[test]
    fn distinct_query_quotes_identifiers() {
        let q = distinct_query("orders", "status").unwrap();
        assert_eq!(
            q,
            "SELECT DISTINCT \"status\" AS v FROM \"orders\" WHERE \"status\" IS NOT NULL"
        );
    }

    #[test]
    fn distinct_query_none_for_unsafe_identifier() {
        assert!(distinct_query("orders; DROP", "status").is_none());
        assert!(distinct_query("orders", "col--").is_none());
    }

    #[test]
    fn targets_only_state_fields() {
        let t = targets(&glossary_with_state());
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].entity, "Order");
        assert_eq!(t[0].table, "order");
        assert_eq!(t[0].column, "status");
    }

    #[cfg(not(feature = "database"))]
    #[test]
    fn observed_from_database_is_none_without_feature() {
        assert!(observed_from_database(&glossary_with_state()).is_none());
    }
}
