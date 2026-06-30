import json
from pathlib import Path

from tauto_project_store.models import ContractDocument


def _metadata_path(markdown_path: Path) -> Path:
    return markdown_path.with_suffix(markdown_path.suffix + ".json")


def save_document(root: Path, document: ContractDocument) -> Path:
    document_path = root / document.project_slug / document.path
    document_path.parent.mkdir(parents=True, exist_ok=True)
    document_path.write_text(document.markdown_content, encoding="utf-8")
    _metadata_path(document_path).write_text(
        json.dumps(
            {
                "project_slug": document.project_slug,
                "path": document.path,
                "title": document.title,
                "version": document.version,
            },
            sort_keys=True,
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    return document_path


def load_document(root: Path, project_slug: str, path: str) -> ContractDocument:
    document_path = root / project_slug / path
    metadata = json.loads(_metadata_path(document_path).read_text(encoding="utf-8"))
    return ContractDocument(
        project_slug=metadata["project_slug"],
        path=metadata["path"],
        title=metadata["title"],
        version=metadata["version"],
        markdown_content=document_path.read_text(encoding="utf-8"),
    )
