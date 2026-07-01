use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use tower_http::services::{ServeDir, ServeFile};

use crate::contract_ir::{find_conflict_candidates, ContractIR, ContractSet};
use crate::scanner::scan_path;

struct ServerState {
    contracts_path: PathBuf,
}

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

fn api_err(e: impl std::fmt::Display) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": e.to_string() })),
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

async fn handle_contracts(State(state): State<Arc<ServerState>>) -> ApiResult<ContractsResponse> {
    let (cs, _, files) = scan_path(&state.contracts_path).map_err(api_err)?;
    // Deduplicate by key so multiple fixture dirs don't show duplicates
    let mut seen: HashSet<String> = HashSet::new();
    let items: Vec<ContractItem> = cs
        .contracts
        .iter()
        .filter(|c| seen.insert(format!("{}/{}/{}", c.entity, c.operation, c.case)))
        .map(contract_item)
        .collect();
    Ok(Json(ContractsResponse {
        contracts: items.len(),
        files,
        items,
    }))
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
    // Deduplicate contracts by key (entity/operation/case) — keep first occurrence
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

    // Build a deduplicated ContractSet for conflict detection
    let deduped_set = ContractSet::new(unique.iter().map(|c| (*c).clone()).collect());
    let conflict_candidates = find_conflict_candidates(&deduped_set);
    let conflict_pairs: HashSet<(String, String)> = conflict_candidates
        .iter()
        .map(|c| (c.key_a.clone(), c.key_b.clone()))
        .collect();

    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut idx = 0usize;
    // Track emitted same_op pairs to avoid duplicates
    let mut emitted: HashSet<(String, String)> = HashSet::new();

    // same_op edges: unique contracts sharing entity+operation (but different case)
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
                // Skip self-loops and already-emitted pairs
                if id_a == id_b {
                    continue;
                }
                let pair = (id_a.clone(), id_b.clone());
                let pair_rev = (id_b.clone(), id_a.clone());
                if emitted.contains(&pair) || emitted.contains(&pair_rev) {
                    continue;
                }
                // Skip pairs that are already conflict candidates (conflict edge takes priority)
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

    // conflict edges
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

async fn handle_graph(State(state): State<Arc<ServerState>>) -> ApiResult<GraphResponse> {
    let (cs, _, _) = scan_path(&state.contracts_path).map_err(api_err)?;
    Ok(Json(build_graph(&cs)))
}

// ── POST /api/v1/contracts/upload ─────────────────────────────────────────────

#[derive(Serialize)]
struct UploadResponse {
    filename: String,
    contracts: usize,
    parse_errors: usize,
}

fn sanitize_filename(raw: &str) -> Option<String> {
    // Strip any path components, keep only the final segment.
    let name = std::path::Path::new(raw)
        .file_name()
        .and_then(|n| n.to_str())?
        .to_owned();
    // Enforce .md extension and restrict chars to [a-zA-Z0-9._-].
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
    mut multipart: Multipart,
) -> ApiResult<UploadResponse> {
    while let Some(field) = multipart.next_field().await.map_err(|e| api_err(e))? {
        if field.name() != Some("file") {
            continue;
        }
        let raw_name = field
            .file_name()
            .unwrap_or("upload.md")
            .to_owned();
        let filename = sanitize_filename(&raw_name)
            .ok_or_else(|| api_err("Filename must be *.md with only alphanumeric, dash, dot, or underscore characters"))?;

        let bytes = field.bytes().await.map_err(|e| api_err(e))?;
        let content = std::str::from_utf8(&bytes)
            .map_err(|_| api_err("File must be valid UTF-8"))?;

        let dest = state.contracts_path.join(&filename);
        std::fs::write(&dest, content).map_err(api_err)?;

        let (cs, parse_errors, _) = scan_path(&dest).map_err(api_err)?;
        return Ok(Json(UploadResponse {
            filename,
            contracts: cs.contracts.len(),
            parse_errors,
        }));
    }
    Err(api_err("No field named 'file' found in the multipart body"))
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
    let state = Arc::new(ServerState { contracts_path: contracts_path.clone() });
    let index_html = ui_dist.join("index.html");
    let serve_dir =
        ServeDir::new(&ui_dist).not_found_service(ServeFile::new(index_html));

    // No CORS middleware: the SPA is served by the same process (same-origin).
    // Adding permissive CORS would allow any website to exfiltrate local contracts.
    let app = Router::new()
        .route("/api/v1/contracts", get(handle_contracts))
        .route("/api/v1/contracts/upload", post(handle_upload))
        .route("/api/v1/graph", get(handle_graph))
        .with_state(state)
        .fallback_service(serve_dir);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    eprintln!("tauto serve → http://localhost:{port}");
    eprintln!("  contracts : {}", contracts_path.display());
    eprintln!("  ui        : {}", ui_dist.display());

    axum::serve(listener, app).await?;
    Ok(())
}
