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
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use tokio::sync::Mutex as AsyncMutex;

use std::path::{Component, Path};

use crate::lean_gen::{run_lake_build_bounded, LakeBuildRequest, LakeBuildResult, LeanWorkspace};

/// Max seconds a single build may run before the worker terminates it (it holds
/// the build lock, so an unbounded build would stall every later request).
const BUILD_TIMEOUT_SECS: u64 = 150;

/// Reject any workspace path that could escape the build tempdir. The `/build`
/// body is attacker-controlled, so an absolute path or a `..` component must not
/// be written (path traversal → arbitrary file write as the worker user).
fn path_is_safe(p: &str) -> bool {
    let path = Path::new(p);
    !path.is_absolute()
        && !p.is_empty()
        && path.components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

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
    // `/v1/build` is the versioned contract; `/build` is kept as an unversioned
    // alias for back-compat with existing TAUTO_LAKE_URL settings.
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/v1/health", get(|| async { "ok" }))
        .route("/build", post(handle_build))
        .route("/v1/build", post(handle_build))
        .with_state(state);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    eprintln!("tauto lake-worker → http://0.0.0.0:{port}  (POST /v1/build, GET /v1/health)");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_build(
    State(state): State<Arc<WorkerState>>,
    Json(req): Json<LakeBuildRequest>,
) -> Response {
    // One build at a time — a memory-heavy lake build shouldn't run N-fold.
    // try_lock (not lock): if a build is already running, fail fast with 503
    // instead of queueing callers behind a 150s build past their own timeouts.
    // The 503 is transient, so advertise Retry-After.
    let _guard = match state.build_lock.try_lock() {
        Ok(g) => g,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                [(header::RETRY_AFTER, "5")],
                Json(LakeBuildResult {
                    success: false,
                    stdout: String::new(),
                    stderr: "build worker busy — another build is in progress".to_owned(),
                }),
            )
                .into_response()
        }
    };
    let workspace = LeanWorkspace { files: req.files };

    let result = tokio::task::spawn_blocking(move || build(workspace))
        .await
        .unwrap_or_else(|e| LakeBuildResult {
            success: false,
            stdout: String::new(),
            stderr: format!("worker task failed: {e}"),
        });
    (StatusCode::OK, Json(result)).into_response()
}

/// Write the workspace to a temp dir and run `lake build`. Any failure is
/// returned as a non-success result (never a panic), so the caller always gets
/// a well-formed response.
#[cfg(test)]
mod tests {
    use super::path_is_safe;

    #[test]
    fn rejects_traversal_and_absolute_paths() {
        assert!(!path_is_safe("../../.ssh/authorized_keys"));
        assert!(!path_is_safe("/etc/passwd"));
        assert!(!path_is_safe("a/../../b"));
        assert!(!path_is_safe(""));
    }

    #[test]
    fn accepts_normal_relative_paths() {
        assert!(path_is_safe("lakefile.toml"));
        assert!(path_is_safe("TautoContracts/contracts/Foo.lean"));
        assert!(path_is_safe("./TautoContracts.lean"));
    }
}

fn build(workspace: LeanWorkspace) -> LakeBuildResult {
    // Reject unsafe paths before writing anything (the request is untrusted).
    if let Some(bad) = workspace.files.iter().find(|f| !path_is_safe(&f.path)) {
        return LakeBuildResult {
            success: false,
            stdout: String::new(),
            stderr: format!("rejected unsafe workspace path: {}", bad.path),
        };
    }
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
    match run_lake_build_bounded(dir.path(), BUILD_TIMEOUT_SECS) {
        Ok(r) => r,
        Err(e) => LakeBuildResult {
            success: false,
            stdout: String::new(),
            stderr: format!("lake build error: {e}"),
        },
    }
}
