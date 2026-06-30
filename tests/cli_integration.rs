use assert_cmd::Command;
use predicates::prelude::*;

fn tauto() -> Command {
    Command::cargo_bin("tauto").unwrap()
}

fn fixture(relative: &str) -> String {
    format!("tests/fixtures/{relative}")
}

// ── verify ────────────────────────────────────────────────────────────────────

#[test]
fn verify_succeeds_on_valid_contracts() {
    let out = tempfile::tempdir().unwrap();
    tauto()
        .args(["verify", &fixture("orders.md"), "--output", out.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Parsed 2 contract(s)"));
}

#[test]
fn verify_strict_exits_one_when_sorry_stubs_present() {
    let out = tempfile::tempdir().unwrap();
    tauto()
        .args([
            "verify",
            &fixture("orders.md"),
            "--output",
            out.path().to_str().unwrap(),
            "--strict",
        ])
        .assert()
        .failure();
}

#[test]
fn verify_reports_conflict_candidates() {
    let out = tempfile::tempdir().unwrap();
    tauto()
        .args(["verify", &fixture("conflicts.md"), "--output", out.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Conflict candidates"));
}

#[test]
fn verify_writes_lean_workspace_to_output_dir() {
    let out = tempfile::tempdir().unwrap();
    tauto()
        .args(["verify", &fixture("orders.md"), "--output", out.path().to_str().unwrap()])
        .assert()
        .success();
    // workspace must contain at least the lakefile
    assert!(out.path().join("lakefile.toml").exists());
}

#[test]
fn verify_format_json_outputs_valid_json() {
    let out = tempfile::tempdir().unwrap();
    let output = tauto()
        .args([
            "verify",
            &fixture("orders.md"),
            "--output",
            out.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(output).unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).expect("stdout must be valid JSON");
    assert_eq!(v["contracts"], 2);
    assert_eq!(v["files"], 1);
    assert!(v["sorry_count"].as_u64().unwrap() > 0);
    assert!(v["conflicts"].is_array());
}

// ── hash ──────────────────────────────────────────────────────────────────────

#[test]
fn hash_prints_semantic_and_provenance() {
    tauto()
        .args(["hash", &fixture("orders.md")])
        .assert()
        .success()
        .stdout(predicate::str::contains("semantic"))
        .stdout(predicate::str::contains("provenance"));
}

#[test]
fn hash_format_json_outputs_valid_json() {
    let output = tauto()
        .args(["hash", &fixture("orders.md"), "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(output).unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).expect("stdout must be valid JSON");
    assert_eq!(v["contracts"], 2);
    assert_eq!(v["files"], 1);
    assert!(v["semantic"].is_string());
    assert!(v["provenance"].is_string());
    // semantic hash excludes source — two identical semantic hashes must match
    let s1 = v["semantic"].as_str().unwrap();
    assert!(!s1.is_empty());
}

#[test]
fn hash_semantic_is_stable_across_path_changes() {
    // Same contract bodies at different file paths:
    // - semantic hash (excludes source) must be equal
    // - provenance hash (includes document_path) must differ
    let out1 = tauto()
        .args(["hash", &fixture("base/orders.md"), "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let out2 = tauto()
        .args(["hash", &fixture("mirror/orders.md"), "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let v1: serde_json::Value = serde_json::from_str(&String::from_utf8(out1).unwrap()).unwrap();
    let v2: serde_json::Value = serde_json::from_str(&String::from_utf8(out2).unwrap()).unwrap();
    assert_eq!(v1["semantic"], v2["semantic"], "semantic hash must be path-independent");
    assert_ne!(v1["provenance"], v2["provenance"], "provenance hash must capture document_path");
}

// ── list ─────────────────────────────────────────────────────────────────────

#[test]
fn list_shows_entity_operation_case() {
    tauto()
        .args(["list", &fixture("orders.md")])
        .assert()
        .success()
        .stdout(predicate::str::contains("Order/cancelOrder/CancelPaidOrder"))
        .stdout(predicate::str::contains("Order/shipOrder/ShipApprovedOrder"));
}

#[test]
fn list_recursive_finds_contracts_in_subdirectory() {
    tauto()
        .args(["list", &fixture("expanded")])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 contract(s)"));
}

// ── diff ─────────────────────────────────────────────────────────────────────

#[test]
fn diff_identical_sets_reports_no_changes() {
    tauto()
        .args(["diff", &fixture("orders.md"), &fixture("orders.md")])
        .assert()
        .success()
        .stdout(predicate::str::contains("No structural changes"));
}

#[test]
fn diff_expansion_reports_expansion_only_yes() {
    tauto()
        .args(["diff", &fixture("base"), &fixture("expanded")])
        .assert()
        .success()
        .stdout(predicate::str::contains("Expansion only: yes"))
        .stdout(predicate::str::contains("+ Order/shipOrder/ShipApprovedOrder"));
}

#[test]
fn diff_narrowing_reports_expansion_only_no() {
    tauto()
        .args(["diff", &fixture("expanded"), &fixture("base")])
        .assert()
        .success()
        .stdout(predicate::str::contains("Expansion only: no"));
}

#[test]
fn diff_strict_exits_one_when_not_expansion_only() {
    tauto()
        .args(["diff", &fixture("expanded"), &fixture("base"), "--strict"])
        .assert()
        .failure();
}

#[test]
fn diff_strict_exits_zero_when_expansion_only() {
    tauto()
        .args(["diff", &fixture("base"), &fixture("expanded"), "--strict"])
        .assert()
        .success();
}
