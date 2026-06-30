use std::collections::HashSet;
use std::path::Path;

use super::workspace::LeanWorkspace;

#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("duplicate workspace paths: {0:?}")]
    DuplicatePaths(Vec<String>),
    #[error("io error writing {path}: {source}")]
    Io { path: String, source: std::io::Error },
}

pub fn write_lean_workspace(workspace: &LeanWorkspace, base_path: &Path) -> Result<(), WriteError> {
    // Assert uniqueness before writing anything — fail loudly rather than silently overwrite
    let mut seen = HashSet::new();
    let mut duplicates: Vec<String> = Vec::new();
    for file in &workspace.files {
        if !seen.insert(&file.path) {
            duplicates.push(file.path.clone());
        }
    }
    if !duplicates.is_empty() {
        return Err(WriteError::DuplicatePaths(duplicates));
    }

    for file in &workspace.files {
        let dest = base_path.join(&file.path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| WriteError::Io { path: file.path.clone(), source: e })?;
        }
        std::fs::write(&dest, &file.content)
            .map_err(|e| WriteError::Io { path: file.path.clone(), source: e })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract_ir::{ContractIR, ContractSet};
    use crate::lean_gen::workspace::{LeanWorkspace, LeanWorkspaceFile, generate_lean_workspace};

    fn minimal_set() -> ContractSet {
        ContractSet::new(vec![ContractIR::new("CancelPaidOrder", "Order", "cancelOrder")])
    }

    #[test]
    fn write_creates_expected_files_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let ws = generate_lean_workspace(&minimal_set());
        write_lean_workspace(&ws, dir.path()).unwrap();

        let lake = dir.path().join("lakefile.toml");
        assert!(lake.exists(), "lakefile.toml must be written");

        let main = dir.path().join("TautoContracts.lean");
        assert!(main.exists(), "TautoContracts.lean must be written");

        let contract = dir.path().join("contracts").join("CancelPaidOrder.lean");
        assert!(contract.exists(), "contract file must be written");
    }

    #[test]
    fn written_content_matches_workspace_content() {
        let dir = tempfile::tempdir().unwrap();
        let ws = generate_lean_workspace(&minimal_set());
        write_lean_workspace(&ws, dir.path()).unwrap();

        let lake_file = ws.files.iter().find(|f| f.path == "lakefile.toml").unwrap();
        let on_disk = std::fs::read_to_string(dir.path().join("lakefile.toml")).unwrap();
        assert_eq!(on_disk, lake_file.content);
    }

    #[test]
    fn duplicate_paths_returns_error_before_writing() {
        let dir = tempfile::tempdir().unwrap();
        let ws = LeanWorkspace {
            files: vec![
                LeanWorkspaceFile { path: "dup.lean".to_owned(), content: "a".to_owned() },
                LeanWorkspaceFile { path: "dup.lean".to_owned(), content: "b".to_owned() },
            ],
        };
        let result = write_lean_workspace(&ws, dir.path());
        assert!(
            matches!(result, Err(WriteError::DuplicatePaths(_))),
            "must reject before writing any file"
        );
        assert!(!dir.path().join("dup.lean").exists(), "no files written when duplicate detected");
    }

    #[test]
    fn write_creates_contracts_subdirectory() {
        let dir = tempfile::tempdir().unwrap();
        let ws = generate_lean_workspace(&minimal_set());
        write_lean_workspace(&ws, dir.path()).unwrap();
        assert!(dir.path().join("contracts").is_dir());
    }
}
