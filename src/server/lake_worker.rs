//! Reference implementation of the generic Lean build service.
//!
//! Exposes a tiny HTTP contract that *any* Lake deployment could implement:
//!
//! ```text
//! POST /build   { "files": [ { "path": "...", "content": "..." }, ... ] }
//!            -> { "success": bool, "stdout": "...", "stderr": "..." }
//! GET  /health  -> 200 OK
//! ```
//!
//! It writes the posted workspace to a fresh temp dir and runs `lake build`
//! against whatever `lake`/Lean toolchain is on PATH, so it is toolchain- and
//! version-agnostic. Builds are serialized (one at a time) so concurrent
//! requests can't stack multiple memory-hungry `lake build`s and OOM the pod.

use std::sync::Arc;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use tokio::sync::Mutex as AsyncMutex;

use crate::lean_gen::{run_lake_build, LakeBuildRequest, LakeBuildResult, LeanWorkspace};

struct WorkerState {
    /// Serializes builds — one `lake build` at a time.
    build_lock: AsyncMutex<()>,
}

pub fn run_lake_worker(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(serve(port))
}

async fn serve(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(WorkerState { build_lock: AsyncMutex::new(()) });
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/build", post(handle_build))
        .with_state(state);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    eprintln!("tauto lake-worker → http://0.0.0.0:{port}  (POST /build, GET /health)");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_build(
    State(state): State<Arc<WorkerState>>,
    Json(req): Json<LakeBuildRequest>,
) -> Json<LakeBuildResult> {
    // One build at a time — a memory-heavy lake build shouldn't run N-fold.
    let _guard = state.build_lock.lock().await;
    let workspace = LeanWorkspace { files: req.files };

    let result = tokio::task::spawn_blocking(move || build(workspace))
        .await
        .unwrap_or_else(|e| LakeBuildResult {
            success: false,
            stdout: String::new(),
            stderr: format!("worker task failed: {e}"),
        });
    Json(result)
}

/// Write the workspace to a temp dir and run `lake build`. Any failure is
/// returned as a non-success result (never a panic), so the caller always gets
/// a well-formed response.
fn build(workspace: LeanWorkspace) -> LakeBuildResult {
    let dir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => {
            return LakeBuildResult {
                success: false,
                stdout: String::new(),
                stderr: format!("could not create build dir: {e}"),
            }
        }
    };
    if let Err(e) = crate::lean_gen::write_lean_workspace(&workspace, dir.path()) {
        return LakeBuildResult {
            success: false,
            stdout: String::new(),
            stderr: format!("could not write workspace: {e}"),
        };
    }
    match run_lake_build(dir.path()) {
        Ok(r) => r,
        Err(e) => LakeBuildResult {
            success: false,
            stdout: String::new(),
            stderr: format!("lake build error: {e}"),
        },
    }
}
