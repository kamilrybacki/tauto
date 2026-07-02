use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::{Multipart, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex as AsyncMutex;
use tower_http::services::{ServeDir, ServeFile};

use crate::contract_ir::{find_conflict_candidates, ContractIR, ContractSet};
use crate::contract_parser::{extract_contract_blocks, parse_contract_block};
use crate::glossary::odcs_source::OdcsStateSource;
use crate::glossary::{
    self, FileStateSource, Glossary, GlossaryWarning, ObservedDomains, ReconcileReport,
    StateCoverage, StateSource,
};
use crate::lean_gen::{
    generate_lean_workspace, run_lake_build, run_lake_build_remote, write_lean_workspace,
    LakeBuildResult,
};
use crate::scanner::{scan_glossary, scan_path};
use crate::test_gen;

struct ServerState {
    /// slug → contracts directory. A single flat dir is the `default` project.
    projects: std::collections::BTreeMap<String, PathBuf>,
    default_project: String,
    history_lock: AsyncMutex<()>,
    /// Built once at startup and reused (keeps one pooled HTTP client). `None`
    /// when the SLM is not configured → `/translate` returns 503.
    slm_translator: Option<Arc<dyn crate::slm::SlmTranslator>>,
}

/// The `?project=<slug>` query param shared by the project-scoped endpoints.
#[derive(Deserialize, Default)]
struct ProjectQuery {
    project: Option<String>,
}

impl ServerState {
    /// Resolve a project's contracts directory. `None` → the default project.
    /// Unknown slug → 404.
    fn project_path(&self, slug: Option<&str>) -> Result<PathBuf, (StatusCode, Json<serde_json::Value>)> {
        let slug = slug.filter(|s| !s.is_empty()).unwrap_or(&self.default_project);
        self.projects.get(slug).cloned().ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": format!("unknown project: {slug}") })),
            )
        })
    }

    fn history_path(&self, contracts_dir: &Path) -> PathBuf {
        contracts_dir.join("_history.json")
    }
}

fn has_markdown_in(dir: &Path, recursive: bool) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else { return false };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_file() && p.extension().is_some_and(|x| x == "md") {
            return true;
        }
        if recursive && p.is_dir() && has_markdown_in(&p, true) {
            return true;
        }
    }
    false
}

/// Discover projects under `root`. If `root` has top-level `.md` files it is a
/// single flat `default` project (back-compat). Otherwise each immediate subdir
/// that contains contracts (recursively) is a project named by its dir.
fn discover_projects(root: &Path) -> (std::collections::BTreeMap<String, PathBuf>, String) {
    let mut projects = std::collections::BTreeMap::new();
    if !has_markdown_in(root, false) {
        if let Ok(entries) = std::fs::read_dir(root) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() && has_markdown_in(&p, true) {
                    if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                        projects.insert(name.to_owned(), p.clone());
                    }
                }
            }
        }
    }
    if projects.is_empty() {
        projects.insert("default".to_owned(), root.to_path_buf());
        return (projects, "default".to_owned());
    }
    let default = if projects.contains_key("default") {
        "default".to_owned()
    } else {
        projects.keys().next().cloned().unwrap()
    };
    (projects, default)
}

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

fn api_err(e: impl std::fmt::Display) -> (StatusCode, Json<serde_json::Value>) {
    // Log the real error (which may contain filesystem paths or other internal
    // detail) server-side, but return a generic message to the client so nothing
    // sensitive leaks. Domain errors (409 conflict, 422 unprocessable) build their
    // own curated bodies and do not go through this path.
    eprintln!("[api error] {e}");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": "Internal server error" })),
    )
}

// ── /api/v1/contracts ────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ContractsResponse {
    contracts: usize,
    files: usize,
    items: Vec<ContractItem>,
}

#[derive(Serialize)]
struct ContractItem {
    key: String,
    entity: String,
    operation: String,
    case: String,
    requires: Vec<crate::contract_ir::Condition>,
    ensures: Vec<crate::contract_ir::Condition>,
    forbidden: Vec<crate::contract_ir::ForbiddenOperation>,
    preserves: Vec<String>,
    assumes: Vec<String>,
    source: Option<String>,
    requires_count: usize,
    ensures_count: usize,
}

fn contract_item(c: &ContractIR) -> ContractItem {
    ContractItem {
        key: format!("{}/{}/{}", c.entity, c.operation, c.case),
        entity: c.entity.clone(),
        operation: c.operation.clone(),
        case: c.case.clone(),
        requires_count: c.requires.len(),
        ensures_count: c.ensures.len(),
        requires: c.requires.clone(),
        ensures: c.ensures.clone(),
        forbidden: c.forbidden.clone(),
        preserves: c.preserves.clone(),
        assumes: c.assumes.clone(),
        source: c.source.as_ref().map(|s| format!("{}:{}", s.document_path, s.start_line)),
    }
}

#[derive(Serialize)]
struct ProjectInfo {
    slug: String,
    contracts: usize,
    is_default: bool,
}

#[derive(Serialize)]
struct ProjectsResponse {
    projects: Vec<ProjectInfo>,
    default_project: String,
}

async fn handle_projects(State(state): State<Arc<ServerState>>) -> ApiResult<ProjectsResponse> {
    let state2 = Arc::clone(&state);
    let projects = tokio::task::spawn_blocking(move || {
        state2
            .projects
            .iter()
            .map(|(slug, path)| {
                let contracts = scan_path(path).map(|(cs, _, _)| cs.contracts.len()).unwrap_or(0);
                ProjectInfo {
                    slug: slug.clone(),
                    contracts,
                    is_default: *slug == state2.default_project,
                }
            })
            .collect::<Vec<_>>()
    })
    .await
    .map_err(api_err)?;
    Ok(Json(ProjectsResponse { projects, default_project: state.default_project.clone() }))
}

async fn handle_contracts(
    State(state): State<Arc<ServerState>>,
    Query(pq): Query<ProjectQuery>,
) -> ApiResult<ContractsResponse> {
    let contracts_path = state.project_path(pq.project.as_deref())?;
    let (cs, _, files) = scan_path(&contracts_path).map_err(api_err)?;
    let mut seen: HashSet<String> = HashSet::new();
    let items: Vec<ContractItem> = cs
        .contracts
        .iter()
        .filter(|c| seen.insert(format!("{}/{}/{}", c.entity, c.operation, c.case)))
        .map(contract_item)
        .collect();
    Ok(Json(ContractsResponse { contracts: items.len(), files, items }))
}

// ── /api/v1/graph ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct GraphResponse {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
}

#[derive(Serialize)]
struct GraphNode {
    id: String,
    data: GraphNodeData,
}

#[derive(Serialize)]
struct GraphNodeData {
    entity: String,
    operation: String,
    case: String,
    source: Option<String>,
    requires_count: usize,
    ensures_count: usize,
}

#[derive(Serialize)]
struct GraphEdge {
    id: String,
    source: String,
    target: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
}

fn contract_key(c: &ContractIR) -> String {
    format!("{}/{}/{}", c.entity, c.operation, c.case)
}

fn build_graph(cs: &ContractSet) -> GraphResponse {
    let mut seen_keys: HashSet<String> = HashSet::new();
    let unique: Vec<&ContractIR> = cs
        .contracts
        .iter()
        .filter(|c| seen_keys.insert(contract_key(c)))
        .collect();

    let nodes: Vec<GraphNode> = unique
        .iter()
        .map(|c| GraphNode {
            id: contract_key(c),
            data: GraphNodeData {
                entity: c.entity.clone(),
                operation: c.operation.clone(),
                case: c.case.clone(),
                source: c.source.as_ref().map(|s| format!("{}:{}", s.document_path, s.start_line)),
                requires_count: c.requires.len(),
                ensures_count: c.ensures.len(),
            },
        })
        .collect();

    let deduped_set = ContractSet::new(unique.iter().map(|c| (*c).clone()).collect());
    let conflict_candidates = find_conflict_candidates(&deduped_set);
    let conflict_pairs: HashSet<(String, String)> = conflict_candidates
        .iter()
        .map(|c| (c.key_a.clone(), c.key_b.clone()))
        .collect();

    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut idx = 0usize;
    let mut emitted: HashSet<(String, String)> = HashSet::new();

    let mut by_op: HashMap<String, Vec<&ContractIR>> = HashMap::new();
    for c in &unique {
        by_op.entry(format!("{}::{}", c.entity, c.operation)).or_default().push(c);
    }
    for group in by_op.values() {
        if group.len() < 2 {
            continue;
        }
        for i in 0..group.len() {
            for j in (i + 1)..group.len() {
                let id_a = contract_key(group[i]);
                let id_b = contract_key(group[j]);
                if id_a == id_b {
                    continue;
                }
                let pair = (id_a.clone(), id_b.clone());
                let pair_rev = (id_b.clone(), id_a.clone());
                if emitted.contains(&pair) || emitted.contains(&pair_rev) {
                    continue;
                }
                if conflict_pairs.contains(&pair) || conflict_pairs.contains(&pair_rev) {
                    continue;
                }
                emitted.insert(pair);
                edges.push(GraphEdge {
                    id: format!("e{idx}"),
                    source: id_a,
                    target: id_b,
                    kind: "same_op".to_owned(),
                    label: None,
                });
                idx += 1;
            }
        }
    }

    for c in &conflict_candidates {
        edges.push(GraphEdge {
            id: format!("e{idx}"),
            source: c.key_a.clone(),
            target: c.key_b.clone(),
            kind: "conflict".to_owned(),
            label: Some(c.reason.clone()),
        });
        idx += 1;
    }

    GraphResponse { nodes, edges }
}

async fn handle_graph(
    State(state): State<Arc<ServerState>>,
    Query(pq): Query<ProjectQuery>,
) -> ApiResult<GraphResponse> {
    let contracts_path = state.project_path(pq.project.as_deref())?;
    let (cs, _, _) = scan_path(&contracts_path).map_err(api_err)?;
    Ok(Json(build_graph(&cs)))
}

// ── /api/v1/history ───────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
enum HistoryOutcome {
    Accepted,
    Rejected,
}

#[derive(Serialize, Deserialize, Clone)]
struct ConflictInfo {
    key_a: String,
    key_b: String,
    reason: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct HistoryEntry {
    id: u64,
    timestamp_unix: u64,
    filename: String,
    outcome: HistoryOutcome,
    contracts_count: usize,
    parse_errors: usize,
    #[serde(default)]
    conflicts: Vec<ConflictInfo>,
}

#[derive(Serialize, Deserialize, Default)]
struct HistoryFile {
    entries: Vec<HistoryEntry>,
}

fn read_history(path: &Path) -> Vec<HistoryEntry> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<HistoryFile>(&s).ok())
        .map(|h| h.entries)
        .unwrap_or_default()
}

fn write_history(path: &Path, entries: &[HistoryEntry]) {
    let file = HistoryFile { entries: entries.to_vec() };
    if let Ok(json) = serde_json::to_string_pretty(&file) {
        let _ = std::fs::write(path, json);
    }
}

async fn record_history(
    state: &Arc<ServerState>,
    contracts_dir: &Path,
    filename: String,
    outcome: HistoryOutcome,
    contracts_count: usize,
    parse_errors: usize,
    conflicts: Vec<ConflictInfo>,
) {
    let history_path = state.history_path(contracts_dir);
    let _guard = state.history_lock.lock().await;
    let _ = tokio::task::spawn_blocking(move || {
        let timestamp_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut entries = read_history(&history_path);
        let id = entries.len() as u64 + 1;
        entries.push(HistoryEntry { id, timestamp_unix, filename, outcome, contracts_count, parse_errors, conflicts });
        write_history(&history_path, &entries);
    })
    .await;
}

#[derive(Serialize)]
struct HistoryResponse {
    entries: Vec<HistoryEntry>,
}

async fn handle_history(
    State(state): State<Arc<ServerState>>,
    Query(pq): Query<ProjectQuery>,
) -> ApiResult<HistoryResponse> {
    let history_path = state.history_path(&state.project_path(pq.project.as_deref())?);
    let entries = tokio::task::spawn_blocking(move || read_history(&history_path))
        .await
        .map_err(|e| api_err(e.to_string()))?;
    let mut reversed = entries;
    reversed.reverse();
    Ok(Json(HistoryResponse { entries: reversed }))
}

// ── /api/v1/proofs ────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct LeanFileEntry {
    path: String,
    content: String,
}

#[derive(Serialize)]
struct ProofsResponse {
    contracts: usize,
    sorry_count: usize,
    files: Vec<LeanFileEntry>,
    build_available: bool,
    build_success: bool,
    build_stdout: String,
    build_stderr: String,
}

async fn handle_proofs(
    State(state): State<Arc<ServerState>>,
    Query(pq): Query<ProjectQuery>,
) -> ApiResult<ProofsResponse> {
    let contracts_path = state.project_path(pq.project.as_deref())?;
    let (cs, _, _) = scan_path(&contracts_path).map_err(api_err)?;

    if cs.contracts.is_empty() {
        return Ok(Json(ProofsResponse {
            contracts: 0,
            sorry_count: 0,
            files: vec![],
            build_available: false,
            build_success: false,
            build_stdout: String::new(),
            build_stderr: "No contracts loaded — upload at least one contract first.".to_owned(),
        }));
    }

    let workspace = generate_lean_workspace(&cs);
    let sorry_count = workspace
        .files
        .iter()
        .map(|f| f.content.matches("sorry").count())
        .sum();
    let files: Vec<LeanFileEntry> = workspace
        .files
        .iter()
        .map(|f| LeanFileEntry { path: f.path.clone(), content: f.content.clone() })
        .collect();

    let build_dir = tempfile::tempdir().map_err(|e| api_err(e.to_string()))?;
    let build_path = build_dir.path().to_path_buf();
    let workspace_clone = workspace;
    // Prefer a remote Lean build service (any deployment implementing the build
    // contract) when TAUTO_LAKE_URL is set; else fall back to a local `lake` on
    // PATH; else report unavailable. Service errors degrade gracefully to
    // build_available=false — never a 500, so a slow/down builder can't hang the
    // proofs request or trip the liveness probe.
    let lake_url = std::env::var("TAUTO_LAKE_URL").ok();

    let (build_available, result): (bool, LakeBuildResult) =
        tokio::task::spawn_blocking(move || {
            if let Some(url) = lake_url {
                return match run_lake_build_remote(&url, &workspace_clone, Duration::from_secs(180)) {
                    Ok(r) => (true, r),
                    Err(e) => (
                        false,
                        LakeBuildResult {
                            success: false,
                            stdout: String::new(),
                            stderr: format!("lake build service unavailable: {e}"),
                        },
                    ),
                };
            }
            if let Err(e) = write_lean_workspace(&workspace_clone, &build_path) {
                return (
                    false,
                    LakeBuildResult { success: false, stdout: String::new(), stderr: e.to_string() },
                );
            }
            match run_lake_build(&build_path) {
                Ok(r) => (true, r),
                Err(_) => (
                    false,
                    LakeBuildResult {
                        success: false,
                        stdout: String::new(),
                        stderr: "lake not found in PATH — set TAUTO_LAKE_URL to a Lean build service, or install Lean 4 via elan".to_owned(),
                    },
                ),
            }
        })
        .await
        .map_err(|e| api_err(e.to_string()))?;

    let build_success = result.success;
    let build_stdout = result.stdout;
    let build_stderr = result.stderr;

    Ok(Json(ProofsResponse {
        contracts: cs.contracts.len(),
        sorry_count,
        files,
        build_available,
        build_success,
        build_stdout,
        build_stderr,
    }))
}

// ── POST /api/v1/contracts/upload ─────────────────────────────────────────────

#[derive(Serialize)]
struct UploadResponse {
    filename: String,
    contracts: usize,
    parse_errors: usize,
}

fn sanitize_filename(raw: &str) -> Option<String> {
    let name = std::path::Path::new(raw)
        .file_name()
        .and_then(|n| n.to_str())?
        .to_owned();
    if !name.ends_with(".md") {
        return None;
    }
    if name.chars().all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-')) {
        Some(name)
    } else {
        None
    }
}

async fn handle_upload(
    State(state): State<Arc<ServerState>>,
    Query(pq): Query<ProjectQuery>,
    mut multipart: Multipart,
) -> ApiResult<UploadResponse> {
    let contracts_path = state.project_path(pq.project.as_deref())?;
    while let Some(field) = multipart.next_field().await.map_err(|e| api_err(e))? {
        if field.name() != Some("file") {
            continue;
        }
        let raw_name = field.file_name().unwrap_or("upload.md").to_owned();
        let filename = sanitize_filename(&raw_name).ok_or_else(|| {
            api_err("Filename must be *.md with only alphanumeric, dash, dot, or underscore characters")
        })?;

        let bytes = field.bytes().await.map_err(|e| api_err(e))?;
        let content = std::str::from_utf8(&bytes)
            .map_err(|_| api_err("File must be valid UTF-8"))?;

        let dest = contracts_path.join(&filename);
        std::fs::write(&dest, content).map_err(api_err)?;

        let (cs, parse_errors, _) = scan_path(&dest).map_err(api_err)?;

        // Conflict gate: reject if this upload introduces a conflict against existing rules.
        let (full_cs, _, _) = scan_path(&contracts_path).map_err(api_err)?;
        let all_conflicts = find_conflict_candidates(&full_cs);
        if !all_conflicts.is_empty() {
            let uploaded_keys: HashSet<String> = cs
                .contracts
                .iter()
                .map(|c| format!("{}/{}/{}", c.entity, c.operation, c.case))
                .collect();
            let introduced: Vec<_> = all_conflicts
                .iter()
                .filter(|c| uploaded_keys.contains(&c.key_a) || uploaded_keys.contains(&c.key_b))
                .collect();
            if !introduced.is_empty() {
                std::fs::remove_file(&dest).ok();
                let conflict_infos: Vec<ConflictInfo> = introduced
                    .iter()
                    .map(|c| ConflictInfo {
                        key_a: c.key_a.clone(),
                        key_b: c.key_b.clone(),
                        reason: c.reason.clone(),
                    })
                    .collect();
                record_history(
                    &state,
                    &contracts_path,
                    filename.clone(),
                    HistoryOutcome::Rejected,
                    cs.contracts.len(),
                    parse_errors,
                    conflict_infos.clone(),
                )
                .await;
                return Err((
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({
                        "error": "Contract conflicts with existing rules",
                        "conflicts": conflict_infos.iter().map(|c| serde_json::json!({
                            "key_a": c.key_a,
                            "key_b": c.key_b,
                            "reason": c.reason
                        })).collect::<Vec<_>>()
                    })),
                ));
            }
        }

        record_history(
            &state,
            &contracts_path,
            filename.clone(),
            HistoryOutcome::Accepted,
            cs.contracts.len(),
            parse_errors,
            vec![],
        )
        .await;

        return Ok(Json(UploadResponse {
            filename,
            contracts: cs.contracts.len(),
            parse_errors,
        }));
    }
    Err(api_err("No field named 'file' found in the multipart body"))
}

// ── /api/v1/check ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct CheckTests {
    total_cases: usize,
    proposed: Vec<test_gen::ContractTestSuite>,
    regression: Vec<test_gen::ContractTestSuite>,
}

#[derive(Serialize)]
struct CheckResponse {
    compatible: bool,
    /// True when no example contradicts its rule (correctness vs stated intent).
    /// Distinct from `compatible` (which is about conflicts with other rules).
    conformant: bool,
    proposed_contracts: usize,
    parse_errors: usize,
    conflicts: Vec<ConflictInfo>,
    /// Advisory glossary findings for the proposed rule (unknown entity/field,
    /// cross-entity references, etc.). Never blocks — informational only.
    glossary_warnings: Vec<GlossaryWarning>,
    /// Per-example conformance outcomes (rule vs its own `examples`).
    conformance: Vec<crate::conformance::ExampleOutcome>,
    /// Rules whose preconditions can never all hold (dead rules).
    dead_rules: Vec<crate::deadrule::DeadRule>,
    tests: CheckTests,
}

async fn handle_check(
    State(state): State<Arc<ServerState>>,
    Query(pq): Query<ProjectQuery>,
    body: String,
) -> ApiResult<CheckResponse> {
    let contracts_path = state.project_path(pq.project.as_deref())?;
    // scan_path (disk I/O), conflict detection (CPU), and test-suite generation
    // (CPU) all run on a blocking thread so the async runtime is not stalled,
    // matching handle_history / handle_proofs.
    tokio::task::spawn_blocking(move || compute_check(&contracts_path, &body))
        .await
        .map_err(api_err)?
        .map(Json)
}

fn dedup_contracts(cs: &ContractSet) -> Vec<ContractIR> {
    let mut seen: HashSet<String> = HashSet::new();
    cs.contracts
        .iter()
        .filter(|c| seen.insert(contract_key(c)))
        .cloned()
        .collect()
}

fn conflict_tuple(c: &crate::contract_ir::ConflictCandidate) -> (String, String, String) {
    (c.key_a.clone(), c.key_b.clone(), c.reason.clone())
}

fn compute_check(
    contracts_path: &Path,
    body: &str,
) -> Result<CheckResponse, (StatusCode, Json<serde_json::Value>)> {
    // Parse proposed content fully in-memory — nothing is written to disk.
    let blocks = extract_contract_blocks(body, "<proposed>");
    let mut proposed_contracts = Vec::new();
    let mut parse_errors = 0usize;
    for block in &blocks {
        let result = parse_contract_block(block);
        parse_errors += result.diagnostics.len();
        if let Some(c) = result.contract {
            proposed_contracts.push(c);
        }
    }

    // A body carrying no parseable contract is a malformed request, not a
    // vacuously-compatible one. Reject it so callers can distinguish "clean
    // and compatible" from "nothing was understood".
    if proposed_contracts.is_empty() {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({
                "error": "No parseable contract block found in request body",
                "parse_errors": parse_errors,
                "blocks_seen": blocks.len(),
            })),
        ));
    }
    let proposed_cs = ContractSet::new(proposed_contracts);

    // Load existing contracts from disk (deduplicated by key, as elsewhere).
    let (existing_raw, _, _) = scan_path(contracts_path).map_err(api_err)?;
    let existing_cs = ContractSet::new(dedup_contracts(&existing_raw));

    // Validate the proposed rule's vocabulary against the domain glossary
    // (advisory — unknown entities/fields, cross-entity references, etc.).
    let glossary = scan_glossary(contracts_path).map_err(api_err)?;
    let glossary_warnings = glossary::validate(&proposed_cs, &glossary);

    // Conflicts already present in the existing set are the baseline; we only
    // report conflicts the proposal *introduces*, so a rule that reuses an
    // existing key does not resurface pre-existing conflicts as if it caused them.
    let baseline: HashSet<(String, String, String)> =
        find_conflict_candidates(&existing_cs).iter().map(conflict_tuple).collect();

    let combined: Vec<ContractIR> = existing_cs
        .contracts
        .iter()
        .chain(proposed_cs.contracts.iter())
        .cloned()
        .collect();
    let combined_cs = ContractSet::new(combined);

    let proposed_keys: HashSet<String> =
        proposed_cs.contracts.iter().map(contract_key).collect();
    let conflicts: Vec<ConflictInfo> = find_conflict_candidates(&combined_cs)
        .iter()
        .filter(|c| !baseline.contains(&conflict_tuple(c)))
        .filter(|c| proposed_keys.contains(&c.key_a) || proposed_keys.contains(&c.key_b))
        .map(|c| ConflictInfo {
            key_a: c.key_a.clone(),
            key_b: c.key_b.clone(),
            reason: c.reason.clone(),
        })
        .collect();

    // Conformance: does each proposed rule agree with its own examples? (rule
    // vs stated intent — correctness, distinct from cross-rule compatibility).
    let conformance = crate::conformance::check_examples(&proposed_cs.contracts);
    let conformant = !conformance
        .iter()
        .any(|o| o.status == crate::conformance::ConformanceStatus::Fail);

    // Dead rules: preconditions that can never all hold (decidable).
    let dead_rules = crate::deadrule::find_dead_rules(&proposed_cs.contracts);

    // Generate JSON test suites — no SLM involved.
    let proposed_suites = test_gen::generate_suite(&proposed_cs);
    let regression_suites = test_gen::generate_suite(&existing_cs);
    let total_cases = proposed_suites.iter().map(|s| s.cases.len()).sum::<usize>()
        + regression_suites.iter().map(|s| s.cases.len()).sum::<usize>();

    Ok(CheckResponse {
        compatible: conflicts.is_empty(),
        conformant,
        proposed_contracts: proposed_cs.contracts.len(),
        parse_errors,
        conflicts,
        glossary_warnings,
        conformance,
        dead_rules,
        tests: CheckTests { total_cases, proposed: proposed_suites, regression: regression_suites },
    })
}

// ── /api/v1/translate ─────────────────────────────────────────────────────────

/// Translate prose business rules into the DSL via the configured SLM provider
/// (default: deterministic stub; live SLM only when opted in via env). Writes
/// nothing and does not check/prove — the caller reviews the returned DSL and
/// then POSTs it to /api/v1/check. This is the SLM front door to the verified
/// pipeline; faithfulness is confirmed at DSL review, not by this call.
/// Optional JSON body for /translate. Clients may POST either raw prose
/// (`text/plain`) or `application/json` with extra context the SLM should use.
#[derive(Deserialize)]
struct TranslateBody {
    prose: String,
    #[serde(default)]
    context: std::collections::BTreeMap<String, String>,
}

async fn handle_translate(
    State(state): State<Arc<ServerState>>,
    Query(pq): Query<ProjectQuery>,
    headers: axum::http::HeaderMap,
    body: String,
) -> ApiResult<crate::slm::TranslationResult> {
    // Accept JSON `{prose, context}` or raw prose (back-compat).
    let is_json = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.starts_with("application/json"));
    let (prose, client_context) = if is_json {
        match serde_json::from_str::<TranslateBody>(&body) {
            Ok(b) => (b.prose, b.context),
            Err(e) => {
                return Err((
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(serde_json::json!({ "error": format!("invalid JSON body: {e}") })),
                ))
            }
        }
    } else {
        (body, std::collections::BTreeMap::new())
    };

    if prose.trim().is_empty() {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({ "error": "empty prose body" })),
        ));
    }
    // Bound the prose: each translation is a paid upstream call, so reject
    // oversized bodies rather than forwarding them to the SLM.
    const MAX_PROSE_BYTES: usize = 16_384;
    if prose.len() > MAX_PROSE_BYTES {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({
                "error": format!("prose too large ({} bytes; max {MAX_PROSE_BYTES})", prose.len())
            })),
        ));
    }
    // Not configured (no SLM at startup) → 503: it's a config problem, and a
    // retry won't help until the operator sets a key.
    let Some(translator) = state.slm_translator.clone() else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "SLM translation is not configured" })),
        ));
    };

    // Provide the glossary vocabulary as context so the SLM uses canonical names;
    // client-supplied context entries are merged on top (and win on conflict).
    let path = state.project_path(pq.project.as_deref())?;
    let glossary = tokio::task::spawn_blocking(move || scan_glossary(&path))
        .await
        .map_err(api_err)?
        .map_err(api_err)?;
    let mut context = std::collections::BTreeMap::new();
    if !glossary.is_empty() {
        if let Ok(g) = serde_json::to_string(&glossary) {
            context.insert("glossary".to_owned(), g);
        }
    }
    context.extend(client_context);

    let result = tokio::task::spawn_blocking(move || {
        translator.translate(&crate::slm::TranslationRequest { prose, context })
    })
    .await
    .map_err(api_err)?;

    match result {
        Ok(r) => Ok(Json(r)),
        // The provider is configured but the upstream call failed — that's a
        // bad-gateway condition (502), distinct from "not configured" (503).
        // Log the detail (which may include the upstream error body); return a
        // generic message so upstream internals don't leak to the client.
        Err(e) => {
            eprintln!("translate: SLM provider error: {e}");
            Err((
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": "SLM translation provider error" })),
            ))
        }
    }
}

// ── /api/v1/glossary ──────────────────────────────────────────────────────────

async fn handle_glossary(
    State(state): State<Arc<ServerState>>,
    Query(pq): Query<ProjectQuery>,
) -> ApiResult<Glossary> {
    let path = state.project_path(pq.project.as_deref())?;
    tokio::task::spawn_blocking(move || scan_glossary(&path))
        .await
        .map_err(api_err)?
        .map(Json)
        .map_err(api_err)
}

// ── /api/v1/lifecycle ─────────────────────────────────────────────────────────

async fn handle_lifecycle(
    State(state): State<Arc<ServerState>>,
    Query(pq): Query<ProjectQuery>,
) -> ApiResult<Vec<StateCoverage>> {
    let path = state.project_path(pq.project.as_deref())?;
    tokio::task::spawn_blocking(move || {
        let (cs, _, _) = scan_path(&path)?;
        let glossary = scan_glossary(&path)?;
        Ok::<_, std::io::Error>(glossary::analyze_lifecycle(&cs, &glossary))
    })
    .await
    .map_err(api_err)?
    .map(Json)
    .map_err(api_err)
}

// ── /api/v1/reconcile ─────────────────────────────────────────────────────────

/// Read observed states from the `_observed_states.json` descriptor in `dir`, or
/// fall back to no observation. A malformed descriptor degrades to "none" rather
/// than erroring the request.
fn file_or_none(dir: &Path) -> (ObservedDomains, &'static str) {
    let file = FileStateSource::new(dir.join("_observed_states.json"));
    if file.exists() {
        match file.observed_domains() {
            Ok(o) => (o, "file"),
            Err(e) => {
                eprintln!("[reconcile:file] {e}");
                (ObservedDomains::new(), "none")
            }
        }
    } else {
        (ObservedDomains::new(), "none")
    }
}

/// File-based source precedence: ODCS data contracts (`*.odcs.yaml`) → native
/// JSON descriptor → none.
fn file_sources(dir: &Path, glossary: &Glossary) -> (ObservedDomains, &'static str) {
    let odcs = OdcsStateSource::new(dir, glossary.clone());
    if odcs.exists() {
        match odcs.observed_domains() {
            Ok(o) => return (o, "odcs"),
            Err(e) => eprintln!("[reconcile:odcs] {e}"),
        }
    }
    file_or_none(dir)
}

/// Reconcile the glossary's declared state domains against observed ones.
/// Source precedence: a live database (when configured) → a `_observed_states.json`
/// descriptor in the contracts dir → none. Advisory: proposes completions, never
/// mutates the glossary.
async fn handle_reconcile(
    State(state): State<Arc<ServerState>>,
    Query(pq): Query<ProjectQuery>,
) -> ApiResult<ReconcileReport> {
    let path = state.project_path(pq.project.as_deref())?;
    tokio::task::spawn_blocking(move || {
        let glossary = scan_glossary(&path)?;

        // Source precedence: live database (when the `database` feature is built
        // and DATABASE_URL is set) → file descriptor → none. A configured source
        // that errors falls through to the next rather than failing the request.
        let (observed, source) = match glossary::db_source::observed_from_database(&glossary) {
            Some(Ok(o)) => (o, "database"),
            Some(Err(e)) => {
                eprintln!("[reconcile:db] {e}");
                file_sources(&path, &glossary)
            }
            None => file_sources(&path, &glossary),
        };

        Ok::<_, std::io::Error>(glossary::reconcile(&glossary, &observed, source))
    })
    .await
    .map_err(api_err)?
    .map(Json)
    .map_err(api_err)
}

// ── entry point ───────────────────────────────────────────────────────────────

pub fn run_serve(
    contracts_path: PathBuf,
    port: u16,
    ui_dist: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(serve_inner(contracts_path, port, ui_dist))
}

async fn serve_inner(
    contracts_path: PathBuf,
    port: u16,
    ui_dist: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let (projects, default_project) = discover_projects(&contracts_path);
    // Build the SLM translator once (reuses its pooled HTTP client). None when
    // unconfigured — /translate then 503s.
    let slm_translator: Option<Arc<dyn crate::slm::SlmTranslator>> =
        match crate::slm::translator_from_env() {
            Ok(t) => Some(Arc::from(t)),
            Err(e) => {
                eprintln!("SLM translation not configured: {e} (/api/v1/translate will 503)");
                None
            }
        };
    let projects_log: Vec<String> = projects.keys().cloned().collect();
    let state = Arc::new(ServerState {
        projects,
        default_project: default_project.clone(),
        history_lock: AsyncMutex::new(()),
        slm_translator,
    });

    let index_html = ui_dist.join("index.html");
    let serve_dir = ServeDir::new(&ui_dist).not_found_service(ServeFile::new(index_html));

    // No CORS middleware: the SPA is served by the same process (same-origin).
    let app = Router::new()
        .route("/api/v1/projects", get(handle_projects))
        .route("/api/v1/contracts", get(handle_contracts))
        .route("/api/v1/contracts/upload", post(handle_upload))
        .route("/api/v1/graph", get(handle_graph))
        .route("/api/v1/history", get(handle_history))
        .route("/api/v1/proofs", get(handle_proofs))
        .route("/api/v1/check", post(handle_check))
        .route("/api/v1/translate", post(handle_translate))
        .route("/api/v1/glossary", get(handle_glossary))
        .route("/api/v1/lifecycle", get(handle_lifecycle))
        .route("/api/v1/reconcile", get(handle_reconcile))
        .with_state(state)
        .fallback_service(serve_dir);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    eprintln!("tauto serve → http://localhost:{port}");
    eprintln!("  projects  : {} (default: {default_project})", projects_log.join(", "));
    eprintln!("  ui        : {}", ui_dist.display());

    axum::serve(listener, app).await?;
    Ok(())
}
