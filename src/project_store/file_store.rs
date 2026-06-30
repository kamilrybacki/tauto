use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::models::ContractDocument;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("document not found: {0}")]
    NotFound(PathBuf),
}

fn metadata_path(markdown_path: &Path) -> PathBuf {
    let mut p = markdown_path.to_path_buf();
    let ext = p.extension().map(|e| {
        let mut s = e.to_os_string();
        s.push(".json");
        s
    });
    p.set_extension(ext.unwrap_or_else(|| "json".into()));
    p
}

pub fn save_document(root: &Path, document: &ContractDocument) -> Result<PathBuf, StoreError> {
    let doc_path = root.join(&document.project_slug).join(&document.path);
    if let Some(parent) = doc_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&doc_path, &document.markdown_content)?;

    let mut meta: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
    meta.insert("project_slug", serde_json::Value::String(document.project_slug.clone()));
    meta.insert("path", serde_json::Value::String(document.path.clone()));
    meta.insert("title", serde_json::Value::String(document.title.clone()));
    meta.insert("version", serde_json::Value::Number(document.version.into()));

    let meta_json = serde_json::to_string_pretty(&meta)? + "\n";
    std::fs::write(metadata_path(&doc_path), meta_json)?;

    Ok(doc_path)
}

pub fn load_document(
    root: &Path,
    project_slug: &str,
    path: &str,
) -> Result<ContractDocument, StoreError> {
    let doc_path = root.join(project_slug).join(path);
    let meta_path = metadata_path(&doc_path);

    if !meta_path.exists() {
        return Err(StoreError::NotFound(meta_path));
    }

    let meta_bytes = std::fs::read(&meta_path)?;
    let meta: serde_json::Value = serde_json::from_slice(&meta_bytes)?;

    let markdown_content = std::fs::read_to_string(&doc_path)?;

    Ok(ContractDocument {
        project_slug: meta["project_slug"].as_str().unwrap_or(project_slug).to_owned(),
        path: meta["path"].as_str().unwrap_or(path).to_owned(),
        title: meta["title"].as_str().unwrap_or("").to_owned(),
        version: meta["version"].as_u64().unwrap_or(1) as u32,
        markdown_content,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_store::models::ContractDocument;

    fn sample_doc() -> ContractDocument {
        ContractDocument::new("test-proj", "orders.md", "Orders", "# Orders\n\nContent.")
    }

    #[test]
    fn save_and_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let doc = sample_doc();
        save_document(dir.path(), &doc).unwrap();
        let loaded = load_document(dir.path(), &doc.project_slug, &doc.path).unwrap();
        assert_eq!(loaded, doc);
    }

    #[test]
    fn save_creates_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let doc = ContractDocument::new("proj", "nested/deep/file.md", "Deep", "content");
        let result = save_document(dir.path(), &doc);
        assert!(result.is_ok());
    }

    #[test]
    fn load_nonexistent_returns_not_found_error() {
        let dir = tempfile::tempdir().unwrap();
        let result = load_document(dir.path(), "proj", "missing.md");
        assert!(matches!(result, Err(StoreError::NotFound(_))));
    }

    #[test]
    fn saved_markdown_content_matches() {
        let dir = tempfile::tempdir().unwrap();
        let doc = sample_doc();
        save_document(dir.path(), &doc).unwrap();
        let loaded = load_document(dir.path(), &doc.project_slug, &doc.path).unwrap();
        assert_eq!(loaded.markdown_content, doc.markdown_content);
    }

    #[test]
    fn metadata_written_as_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let doc = sample_doc();
        let doc_path = save_document(dir.path(), &doc).unwrap();
        let meta_bytes = std::fs::read(metadata_path(&doc_path)).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&meta_bytes).unwrap();
        assert_eq!(parsed["title"].as_str(), Some("Orders"));
        assert_eq!(parsed["version"].as_u64(), Some(1));
    }
}
