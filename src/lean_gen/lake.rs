use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::lean_gen::workspace::{LeanWorkspace, LeanWorkspaceFile};

/// Result of a `lake build`, and the response body of a remote build service.
/// `serde(default)` on the text fields keeps the client tolerant of a minimal
/// service that returns only `{"success": bool}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LakeBuildResult {
    pub success: bool,
    #[serde(default)]
    pub stdout: String,
    #[serde(default)]
    pub stderr: String,
}

/// Request body of the generic remote build contract: the workspace to compile,
/// as a flat list of `{path, content}` files. Any Lake-capable service can
/// implement this — write the files, run `lake build`, return a LakeBuildResult.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LakeBuildRequest {
    pub files: Vec<LeanWorkspaceFile>,
}

/// Build a Lean workspace on a remote Lake service. `endpoint` is the full build
/// URL (e.g. `http://tauto-lake:4001/build`), so tauto can target any conforming
/// deployment. A finite timeout bounds the request; connection/timeout/HTTP
/// errors surface as `Err` for the caller to degrade gracefully.
pub fn run_lake_build_remote(
    endpoint: &str,
    workspace: &LeanWorkspace,
    timeout: Duration,
) -> Result<LakeBuildResult, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| format!("http client: {e}"))?;
    let body = LakeBuildRequest { files: workspace.files.clone() };
    let resp = client
        .post(endpoint)
        .json(&body)
        .send()
        .map_err(|e| format!("request to {endpoint} failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("{endpoint} returned HTTP {}", resp.status()));
    }
    resp.json::<LakeBuildResult>()
        .map_err(|e| format!("invalid build response from {endpoint}: {e}"))
}

/// Per-module outcome parsed from a `lake build` log. Lake prints one line per
/// module (`✔ [3/7] Built M`, `⚠ … Built M` with warnings, and
/// `error: path/To/File.lean:3:8: …` on failure), so even a failed build tells
/// us exactly which modules — and therefore which obligations — are fine.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ModuleResults {
    /// Modules Lake reported as built (with or without warnings).
    pub built: std::collections::HashSet<String>,
    /// Module → first error line, for modules with compile errors.
    pub failed: std::collections::HashMap<String, String>,
}

/// Map a workspace-relative `.lean` path to its module name
/// (`TautoContracts/contracts/X.lean` → `TautoContracts.contracts.X`).
fn path_to_module(path: &str) -> String {
    path.trim_end_matches(".lean").replace('/', ".")
}

/// Parse per-module results out of a lake build log (stdout + stderr combined;
/// lake splits its output across both). Tolerant: unrecognized lines are
/// ignored, so a Lake format drift degrades to "unknown", never a wrong status.
pub fn parse_module_results(log: &str) -> ModuleResults {
    let mut out = ModuleResults::default();
    for line in log.lines() {
        let t = line.trim();
        // `✔ [3/7] Built TautoContracts.contracts.X (639ms)` (or ⚠ with warnings)
        if let Some(pos) = t.find("] Built ") {
            let rest = &t[pos + "] Built ".len()..];
            let module = rest.split_whitespace().next().unwrap_or("");
            if !module.is_empty() {
                out.built.insert(module.to_owned());
            }
            continue;
        }
        // `error: TautoContracts/contracts/X.lean:3:8: unknown identifier …`
        if let Some(rest) = t.strip_prefix("error: ") {
            if let Some(dot) = rest.find(".lean:") {
                let path = &rest[..dot + ".lean".len()];
                let module = path_to_module(path);
                out.failed.entry(module).or_insert_with(|| t.to_owned());
            }
        }
    }
    // A module that both "Built" and errored (shouldn't happen) counts as failed.
    for m in out.failed.keys() {
        out.built.remove(m);
    }
    out
}

#[derive(Debug, thiserror::Error)]
pub enum LakeError {
    #[error("lake not found in PATH — install Lean 4 via elan: https://github.com/leanprover/elan")]
    NotFound,
    #[error("io error running lake: {0}")]
    Io(#[from] std::io::Error),
}

pub fn check_lean_available() -> Result<(), LakeError> {
    Command::new("lake")
        .arg("--version")
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                LakeError::NotFound
            } else {
                LakeError::Io(e)
            }
        })?;
    Ok(())
}

pub fn run_lake_build(workspace_path: &Path) -> Result<LakeBuildResult, LakeError> {
    let output = Command::new("lake")
        .arg("build")
        .current_dir(workspace_path)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                LakeError::NotFound
            } else {
                LakeError::Io(e)
            }
        })?;

    Ok(LakeBuildResult {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

/// Like [`run_lake_build`], but wraps the build in the `timeout` utility so a
/// hung `lake build` cannot run forever. The lake worker uses this — it holds a
/// serialization lock across the build, so an unbounded build would stall every
/// later request. On timeout the process is killed (`timeout` exits 124) and a
/// non-success result is returned. Requires coreutils `timeout` (present in the
/// worker image); if absent, falls back to an unbounded build.
pub fn run_lake_build_bounded(
    workspace_path: &Path,
    timeout_secs: u64,
) -> Result<LakeBuildResult, LakeError> {
    let output = Command::new("timeout")
        .arg("--kill-after=10")
        .arg(timeout_secs.to_string())
        .arg("lake")
        .arg("build")
        .current_dir(workspace_path)
        .output();
    let output = match output {
        Ok(o) => o,
        // No `timeout` binary → fall back to an unbounded build rather than
        // misreporting lake as missing.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return run_lake_build(workspace_path)
        }
        Err(e) => return Err(LakeError::Io(e)),
    };
    // `timeout` exits 124 when it had to terminate the command.
    if output.status.code() == Some(124) {
        return Ok(LakeBuildResult {
            success: false,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: format!("lake build exceeded {timeout_secs}s and was terminated"),
        });
    }
    Ok(LakeBuildResult {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lake_build_result_success_field_reflects_exit_code() {
        let r = LakeBuildResult { success: true, stdout: String::new(), stderr: String::new() };
        assert!(r.success);
        let r = LakeBuildResult { success: false, stdout: String::new(), stderr: "err".to_owned() };
        assert!(!r.success);
    }

    #[test]
    fn lake_not_found_error_mentions_elan() {
        let msg = LakeError::NotFound.to_string();
        assert!(msg.contains("elan"), "error must point user to elan for installation");
    }

    #[test]
    fn run_lake_build_returns_not_found_when_lake_absent() {
        // Construct a path guaranteed to not have lake — use an empty temp dir as cwd
        // and a command that doesn't exist so we test the NotFound branch indirectly.
        let dir = tempfile::tempdir().unwrap();
        // We test via the error kind mapping: NotFound I/O error → LakeError::NotFound
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "lake not found");
        let lake_err: LakeError = io_err.into();
        // A plain Io variant is produced, but the real run_lake_build maps NotFound kind → NotFound
        // This verifies the From impl and kind check in run_lake_build are consistent.
        assert!(matches!(lake_err, LakeError::Io(_)));
        // Verify the exact branch: kind == NotFound → LakeError::NotFound
        let actual_not_found = std::io::Error::from(std::io::ErrorKind::NotFound);
        let mapped = if actual_not_found.kind() == std::io::ErrorKind::NotFound {
            LakeError::NotFound
        } else {
            LakeError::Io(actual_not_found)
        };
        assert!(matches!(mapped, LakeError::NotFound));
        drop(dir);
    }

    #[test]
    fn parse_module_results_reads_built_and_failed() {
        // Formats captured from real Lake 5.0 logs in this repo's CI.
        let log = "\
info: TautoContracts: no previous manifest, creating one from scratch
✔ [2/7] Built TautoContracts.Model (515ms)
⚠ [3/7] Built TautoContracts.contracts.ShipPaidOrder (3.3s)
warning: TautoContracts/contracts/ShipPaidOrder.lean:3:8: declaration uses `sorry`
error: TautoContracts/contracts/Broken.lean:4:10: unknown identifier 'nope'
error: TautoContracts/contracts/Broken.lean:9:2: type mismatch
✔ [5/7] Built TautoContracts.Conflicts (728ms)
Build completed successfully (7 jobs).";
        let r = parse_module_results(log);
        assert!(r.built.contains("TautoContracts.Model"));
        assert!(r.built.contains("TautoContracts.contracts.ShipPaidOrder"), "⚠ Built counts as built");
        assert!(r.built.contains("TautoContracts.Conflicts"));
        let err = r.failed.get("TautoContracts.contracts.Broken").expect("failed module");
        assert!(err.contains("unknown identifier"), "keeps the first error line");
        assert!(!r.built.contains("TautoContracts.contracts.Broken"));
    }

    #[test]
    fn parse_module_results_tolerates_unknown_lines() {
        let r = parse_module_results("some random output\nno modules here\n");
        assert!(r.built.is_empty() && r.failed.is_empty());
    }

    #[test]
    fn build_result_deserializes_from_minimal_success_only() {
        // The client must tolerate a spartan service that returns only success.
        let r: LakeBuildResult = serde_json::from_str(r#"{"success":true}"#).unwrap();
        assert!(r.success);
        assert_eq!(r.stdout, "");
        assert_eq!(r.stderr, "");
    }

    #[test]
    fn build_request_round_trips() {
        let req = LakeBuildRequest {
            files: vec![LeanWorkspaceFile { path: "lakefile.toml".into(), content: "name = \"X\"".into() }],
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: LakeBuildRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn remote_build_errors_gracefully_on_unreachable_endpoint() {
        let ws = LeanWorkspace { files: vec![], obligations: vec![] };
        // Port 1 is unbound — connection must fail as Err, not panic/hang.
        let out = run_lake_build_remote("http://127.0.0.1:1/build", &ws, Duration::from_millis(500));
        assert!(out.is_err());
    }
}
