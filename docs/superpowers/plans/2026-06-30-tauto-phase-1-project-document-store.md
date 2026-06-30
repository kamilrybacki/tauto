# Tauto Phase 1 Project And Document Store Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add small, testable project and document primitives that can store Markdown contracts without requiring a database.

**Architecture:** Keep domain models pure and immutable. Put filesystem IO in one adapter module so later API/database persistence can reuse the same models without coupling parser tests to storage.

**Tech Stack:** Python 3.12, Pydantic v2, pytest, pathlib.

---

## File Structure

- Create: `src/tauto_project_store/models.py` - immutable `Project` and `ContractDocument` models.
- Create: `src/tauto_project_store/file_store.py` - file-backed helpers for local development and examples.
- Modify: `src/tauto_project_store/__init__.py` - export public symbols.
- Create: `tests/project_store/test_models.py` - model behavior tests.
- Create: `tests/project_store/test_file_store.py` - file-backed persistence tests.

## Task 1: Project Model

**Files:**
- Create: `src/tauto_project_store/models.py`
- Test: `tests/project_store/test_models.py`

- [ ] **Step 1: Write the failing model test**

```python
from tauto_project_store.models import Project


def test_project_normalizes_slug_and_defaults_contract_store_type() -> None:
    project = Project(name="Order Service", slug="Order Service")

    assert project.name == "Order Service"
    assert project.slug == "order-service"
    assert project.contract_store_type == "local"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/project_store/test_models.py::test_project_normalizes_slug_and_defaults_contract_store_type -v`

Expected: FAIL with `ModuleNotFoundError` or import error for `Project`.

- [ ] **Step 3: Implement minimal model**

```python
from pydantic import BaseModel, ConfigDict, field_validator


class Project(BaseModel):
    model_config = ConfigDict(frozen=True)

    name: str
    slug: str
    description: str = ""
    default_branch: str = "main"
    contract_store_type: str = "local"

    @field_validator("slug")
    @classmethod
    def normalize_slug(cls, value: str) -> str:
        return value.strip().lower().replace(" ", "-")
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pytest tests/project_store/test_models.py::test_project_normalizes_slug_and_defaults_contract_store_type -v`

Expected: PASS.

## Task 2: Contract Document Model

**Files:**
- Modify: `src/tauto_project_store/models.py`
- Test: `tests/project_store/test_models.py`

- [ ] **Step 1: Write the failing document test**

```python
from tauto_project_store.models import ContractDocument


def test_contract_document_keeps_markdown_and_version() -> None:
    document = ContractDocument(
        project_slug="order-service",
        path="business-cases/orders/cancel-paid-order.md",
        title="Cancel paid order",
        markdown_content="# Cancel paid order\n",
    )

    assert document.project_slug == "order-service"
    assert document.version == 1
    assert document.markdown_content == "# Cancel paid order\n"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/project_store/test_models.py::test_contract_document_keeps_markdown_and_version -v`

Expected: FAIL with import error for `ContractDocument`.

- [ ] **Step 3: Implement minimal document model**

Add to `src/tauto_project_store/models.py`:

```python
class ContractDocument(BaseModel):
    model_config = ConfigDict(frozen=True)

    project_slug: str
    path: str
    title: str
    markdown_content: str
    version: int = 1

    @field_validator("project_slug")
    @classmethod
    def normalize_project_slug(cls, value: str) -> str:
        return value.strip().lower().replace(" ", "-")
```

- [ ] **Step 4: Run model tests**

Run: `pytest tests/project_store/test_models.py -v`

Expected: PASS.

## Task 3: File Store Round Trip

**Files:**
- Create: `src/tauto_project_store/file_store.py`
- Test: `tests/project_store/test_file_store.py`

- [ ] **Step 1: Write the failing file-store test**

```python
from pathlib import Path

from tauto_project_store.file_store import load_document, save_document
from tauto_project_store.models import ContractDocument


def test_save_and_load_document_round_trip(tmp_path: Path) -> None:
    document = ContractDocument(
        project_slug="order-service",
        path="business-cases/orders/cancel-paid-order.md",
        title="Cancel paid order",
        markdown_content="# Cancel paid order\n",
    )

    saved_path = save_document(tmp_path, document)
    loaded = load_document(tmp_path, "order-service", "business-cases/orders/cancel-paid-order.md")

    assert saved_path == tmp_path / "order-service" / "business-cases/orders/cancel-paid-order.md"
    assert loaded == document
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/project_store/test_file_store.py::test_save_and_load_document_round_trip -v`

Expected: FAIL because `tauto_project_store.file_store` does not exist.

- [ ] **Step 3: Implement file-backed helpers**

```python
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
```

- [ ] **Step 4: Run file-store tests**

Run: `pytest tests/project_store/test_file_store.py -v`

Expected: PASS.

## Task 4: Public Exports

**Files:**
- Modify: `src/tauto_project_store/__init__.py`
- Test: `tests/project_store/test_models.py`

- [ ] **Step 1: Write failing public API test**

```python
from tauto_project_store import ContractDocument, Project


def test_project_store_public_exports() -> None:
    assert Project(name="Order Service", slug="order-service").slug == "order-service"
    assert ContractDocument(
        project_slug="order-service",
        path="rules.md",
        title="Rules",
        markdown_content="",
    ).version == 1
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/project_store/test_models.py::test_project_store_public_exports -v`

Expected: FAIL because symbols are not exported.

- [ ] **Step 3: Export public symbols**

```python
from tauto_project_store.models import ContractDocument, Project

__all__ = ["ContractDocument", "Project"]
```

- [ ] **Step 4: Run project-store tests**

Run: `pytest tests/project_store -v`

Expected: PASS.

## Self-Review Checklist

- [ ] Domain models are immutable.
- [ ] Filesystem behavior is isolated in `file_store.py`.
- [ ] No parser, Lean, API, or database behavior is introduced.
- [ ] Tests use `tmp_path` and do not depend on repository state.
