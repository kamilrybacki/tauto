use assert_cmd::Command;
use predicates::prelude::*;

fn tauto() -> Command {
    let mut cmd = Command::cargo_bin("tauto").unwrap();
    // Bypass the startup Lean check for integration tests on machines without Lean installed.
    // Tests that specifically exercise --lean-check must clear this variable themselves.
    cmd.env("TAUTO_SKIP_LEAN_CHECK", "1");
    cmd
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
fn verify_strict_succeeds_when_workspace_is_sorry_free() {
    // The generator now emits real, discharged proofs (no sorry stubs), so
    // --strict — which fails only on remaining stubs — succeeds.
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
        .success();
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
    // Generated obligations are now real proofs, so no sorry stubs remain.
    assert_eq!(v["sorry_count"].as_u64().unwrap(), 0);
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

// ── list --format json ────────────────────────────────────────────────────────

#[test]
fn list_format_json_outputs_valid_json() {
    let output = tauto()
        .args(["list", &fixture("orders.md"), "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).expect("must be valid JSON");
    assert_eq!(v["contracts"], 2);
    assert_eq!(v["files"], 1);
    assert!(v["items"].is_array());
    assert_eq!(v["items"].as_array().unwrap().len(), 2);
    assert_eq!(v["items"][0]["entity"], "Order");
}

#[test]
fn list_format_json_items_have_entity_operation_case() {
    let output = tauto()
        .args(["list", &fixture("orders.md"), "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value =
        serde_json::from_str(&String::from_utf8(output).unwrap()).unwrap();
    let first = &v["items"][0];
    assert!(first["entity"].is_string());
    assert!(first["operation"].is_string());
    assert!(first["case"].is_string());
}

// ── diff --format json ────────────────────────────────────────────────────────

#[test]
fn diff_format_json_expansion_reports_expansion_only_true() {
    let output = tauto()
        .args(["diff", &fixture("base"), &fixture("expanded"), "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value =
        serde_json::from_str(&String::from_utf8(output).unwrap()).unwrap();
    assert_eq!(v["expansion_only"], true);
    assert_eq!(v["added"].as_array().unwrap().len(), 1);
    assert!(v["removed"].as_array().unwrap().is_empty());
}

#[test]
fn diff_format_json_narrowing_reports_expansion_only_false() {
    let output = tauto()
        .args(["diff", &fixture("expanded"), &fixture("base"), "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value =
        serde_json::from_str(&String::from_utf8(output).unwrap()).unwrap();
    assert_eq!(v["expansion_only"], false);
    assert_eq!(v["removed"].as_array().unwrap().len(), 1);
}

#[test]
fn diff_format_json_includes_conflict_candidates_array() {
    let output = tauto()
        .args(["diff", &fixture("conflicts.md"), &fixture("conflicts.md"), "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value =
        serde_json::from_str(&String::from_utf8(output).unwrap()).unwrap();
    assert!(v["conflict_candidates"].is_array());
}

// ── verify --lean-check ───────────────────────────────────────────────────────

#[test]
fn verify_lean_check_fails_gracefully_when_lake_not_in_path() {
    let out = tempfile::tempdir().unwrap();
    tauto()
        // Restrict PATH so lake binary is unreachable; also clear the skip bypass
        .env("PATH", "/usr/bin:/bin")
        .env_remove("TAUTO_SKIP_LEAN_CHECK")
        .args([
            "verify",
            &fixture("orders.md"),
            "--output",
            out.path().to_str().unwrap(),
            "--lean-check",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("lake"));
}

// ── verify --model ────────────────────────────────────────────────────────────

#[test]
fn verify_model_unknown_exits_with_error() {
    let out = tempfile::tempdir().unwrap();
    tauto()
        .args([
            "verify",
            &fixture("orders.md"),
            "--output",
            out.path().to_str().unwrap(),
            "--model",
            "gpt-4-turbo",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown model"));
}

#[test]
fn verify_model_deepseek_without_api_key_exits_with_error() {
    let out = tempfile::tempdir().unwrap();
    tauto()
        .env_remove("DEEPSEEK_API_KEY")
        .args([
            "verify",
            &fixture("orders.md"),
            "--output",
            out.path().to_str().unwrap(),
            "--model",
            "deepseek",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("DEEPSEEK_API_KEY"));
}

// ── store ─────────────────────────────────────────────────────────────────────

#[test]
fn store_creates_document_file_under_project_slug() {
    let store = tempfile::tempdir().unwrap();
    tauto()
        .args([
            "store",
            &fixture("orders.md"),
            "--project",
            "my-project",
            "--store-root",
            store.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("my-project"));
    assert!(store.path().join("my-project").join("orders.md").exists());
}

#[test]
fn store_format_json_reports_stored_paths() {
    let store = tempfile::tempdir().unwrap();
    let output = tauto()
        .args([
            "store",
            &fixture("orders.md"),
            "--project",
            "orders-proj",
            "--store-root",
            store.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).expect("must be valid JSON");
    assert_eq!(v["project"], "orders-proj");
    assert!(v["stored"].is_array());
    assert_eq!(v["stored"].as_array().unwrap().len(), 1);
}

#[test]
fn store_creates_json_sidecar_metadata() {
    let store = tempfile::tempdir().unwrap();
    tauto()
        .args([
            "store",
            &fixture("orders.md"),
            "--project",
            "sidecar-test",
            "--store-root",
            store.path().to_str().unwrap(),
        ])
        .assert()
        .success();
    assert!(store.path().join("sidecar-test").join("orders.md.json").exists());
}

#[test]
fn store_slug_normalizes_spaces_to_hyphens() {
    let store = tempfile::tempdir().unwrap();
    tauto()
        .args([
            "store",
            &fixture("orders.md"),
            "--project",
            "My Project",
            "--store-root",
            store.path().to_str().unwrap(),
        ])
        .assert()
        .success();
    assert!(store.path().join("my-project").exists());
}

#[test]
fn store_slug_strips_path_traversal_characters() {
    let store = tempfile::tempdir().unwrap();
    tauto()
        .args([
            "store",
            &fixture("orders.md"),
            "--project",
            "../outside",
            "--store-root",
            store.path().to_str().unwrap(),
        ])
        .assert()
        .success();
    // "../outside" normalizes to "outside" — the ".." and "/" are stripped
    assert!(store.path().join("outside").exists(), "traversal sequences must be stripped from slug");
    assert!(!store.path().parent().unwrap().join("outside").exists(), "must not escape store root");
}

#[test]
fn store_recursive_preserves_relative_paths_to_avoid_collision() {
    let store = tempfile::tempdir().unwrap();
    // "base" and "mirror" both contain orders.md — relative paths prevent silent overwrite
    tauto()
        .args([
            "store",
            "tests/fixtures",
            "--project",
            "all-contracts",
            "--store-root",
            store.path().to_str().unwrap(),
        ])
        .assert()
        .success();
    let proj = store.path().join("all-contracts");
    // base/orders.md and mirror/orders.md must be stored at distinct paths
    assert!(proj.join("base").join("orders.md").exists(), "base/orders.md must be stored");
    assert!(proj.join("mirror").join("orders.md").exists(), "mirror/orders.md must be stored");
}

// ── retrieve ──────────────────────────────────────────────────────────────────

fn store_and_retrieve(store: &tempfile::TempDir, project: &str, fixture_path: &str) {
    tauto()
        .args(["store", fixture_path, "--project", project, "--store-root", store.path().to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn retrieve_lists_stored_documents() {
    let store = tempfile::tempdir().unwrap();
    store_and_retrieve(&store, "orders-project", &fixture("orders.md"));
    tauto()
        .args(["retrieve", "--project", "orders-project", "--store-root", store.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("orders-project"))
        .stdout(predicate::str::contains("1 document(s)"));
}

#[test]
fn retrieve_format_json_outputs_valid_json() {
    let store = tempfile::tempdir().unwrap();
    store_and_retrieve(&store, "json-proj", &fixture("orders.md"));
    let output = tauto()
        .args(["retrieve", "--project", "json-proj", "--store-root", store.path().to_str().unwrap(), "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_str(&String::from_utf8(output).unwrap()).unwrap();
    assert_eq!(v["project"], "json-proj");
    assert_eq!(v["documents"], 1);
    assert!(v["contracts"].as_u64().unwrap() > 0);
    assert!(v["items"].is_array());
    assert!(v["items"][0]["path"].is_string());
    assert!(v["items"][0]["contracts"].is_number());
}

#[test]
fn retrieve_unknown_project_exits_with_error() {
    let store = tempfile::tempdir().unwrap();
    tauto()
        .args(["retrieve", "--project", "no-such-project", "--store-root", store.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no-such-project"));
}

#[test]
fn retrieve_reports_contract_count_per_document() {
    let store = tempfile::tempdir().unwrap();
    store_and_retrieve(&store, "count-proj", &fixture("orders.md"));
    tauto()
        .args(["retrieve", "--project", "count-proj", "--store-root", store.path().to_str().unwrap()])
        .assert()
        .success()
        // orders.md has 2 contracts
        .stdout(predicate::str::contains("2 contract(s)"));
}
