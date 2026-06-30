use std::path::PathBuf;

use crate::contract_ir::ContractSet;
use crate::contract_parser::{extract_contract_blocks, parse_contract_block};

/// Recursively scan `path` for markdown files, parse all contract blocks, and
/// return `(contract_set, parse_error_count, file_count)`.
pub fn scan_path(path: &std::path::Path) -> std::io::Result<(ContractSet, usize, usize)> {
    let files = collect_markdown_files(path)?;
    let file_count = files.len();
    let mut contracts = Vec::new();
    let mut parse_errors = 0usize;
    for file_path in &files {
        // Skip unreadable files rather than failing the entire scan.
        let content = match std::fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => { parse_errors += 1; continue; }
        };
        let doc_path = file_path.display().to_string();
        for block in &extract_contract_blocks(&content, &doc_path) {
            let result = parse_contract_block(block);
            parse_errors += result.diagnostics.len();
            if let Some(contract) = result.contract {
                contracts.push(contract);
            }
        }
    }
    Ok((ContractSet::new(contracts), parse_errors, file_count))
}

pub fn collect_markdown_files(path: &std::path::Path) -> std::io::Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    let mut files = Vec::new();
    collect_recursive(path, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_recursive(dir: &std::path::Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        // Use DirEntry::file_type() (no extra syscall, does NOT follow symlinks)
        // to avoid infinite recursion on circular symlinks.
        let ft = entry.file_type()?;
        if ft.is_dir() {
            collect_recursive(&p, files)?;
        } else if ft.is_file() && p.extension().map(|e| e == "md").unwrap_or(false) {
            files.push(p);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write(dir: &TempDir, name: &str, content: &str) {
        let p = dir.path().join(name);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, content).unwrap();
    }

    #[test]
    fn collect_single_file_returns_that_file() {
        let dir = TempDir::new().unwrap();
        write(&dir, "spec.md", "# hello");
        let path = dir.path().join("spec.md");
        let files = collect_markdown_files(&path).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], path);
    }

    #[test]
    fn collect_directory_returns_sorted_md_files() {
        let dir = TempDir::new().unwrap();
        write(&dir, "b.md", "");
        write(&dir, "a.md", "");
        write(&dir, "c.txt", "");
        let files = collect_markdown_files(dir.path()).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files[0].ends_with("a.md"));
        assert!(files[1].ends_with("b.md"));
    }

    #[test]
    fn collect_recursive_finds_nested_md() {
        let dir = TempDir::new().unwrap();
        write(&dir, "top.md", "");
        write(&dir, "sub/nested.md", "");
        write(&dir, "sub/other.txt", "");
        let files = collect_markdown_files(dir.path()).unwrap();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn scan_path_empty_dir_returns_zero_contracts() {
        let dir = TempDir::new().unwrap();
        let (cs, errors, files) = scan_path(dir.path()).unwrap();
        assert_eq!(cs.contracts.len(), 0);
        assert_eq!(errors, 0);
        assert_eq!(files, 0);
    }

    fn contract_md() -> &'static str {
        "```contract\ncase ShipOrder\nentity:\n  Order\noperation:\n  ship\n```"
    }

    #[test]
    fn scan_path_parses_valid_contract_block() {
        let dir = TempDir::new().unwrap();
        write(&dir, "order.md", contract_md());
        let (cs, errors, files) = scan_path(dir.path()).unwrap();
        assert_eq!(files, 1);
        assert_eq!(errors, 0);
        assert_eq!(cs.contracts.len(), 1);
        assert_eq!(cs.contracts[0].case, "ShipOrder");
    }

    #[test]
    fn scan_path_on_single_file() {
        let dir = TempDir::new().unwrap();
        write(&dir, "order.md", contract_md());
        let file = dir.path().join("order.md");
        let (cs, _, files) = scan_path(&file).unwrap();
        assert_eq!(files, 1);
        assert_eq!(cs.contracts.len(), 1);
    }
}
