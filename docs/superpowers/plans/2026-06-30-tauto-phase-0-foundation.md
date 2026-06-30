# Tauto Phase 0 Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create the Tauto repository foundation for isolated, test-driven Python package development.

**Architecture:** Use a Python `src/` layout with small package boundaries. Phase 0 creates only tooling, metadata, and placeholder import modules; behavior lands in later phases through TDD.

**Tech Stack:** Python 3.12, pytest, Pydantic v2, Ruff, pyproject-based packaging.

---

## File Structure

- Create: `pyproject.toml` - project metadata, runtime dependencies, test/lint configuration.
- Create: `README.md` - short project overview and local commands.
- Create: `.gitignore` - Python and local build artifacts.
- Create: `src/tauto_contract_ir/__init__.py` - public namespace for IR package.
- Create: `src/tauto_contract_parser/__init__.py` - public namespace for parser package.
- Create: `src/tauto_project_store/__init__.py` - public namespace for local project/document helpers.
- Create: `tests/test_imports.py` - smoke test proving packages import.

## Task 1: Project Metadata

**Files:**
- Create: `pyproject.toml`

- [ ] **Step 1: Write the project metadata**

```toml
[project]
name = "tauto"
version = "0.1.0"
description = "Lean-backed business contract verification platform"
requires-python = ">=3.12"
dependencies = [
  "pydantic>=2.7,<3",
]

[project.optional-dependencies]
dev = [
  "pytest>=8.2,<9",
  "ruff>=0.5,<1",
]

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[tool.hatch.build.targets.wheel]
packages = [
  "src/tauto_contract_ir",
  "src/tauto_contract_parser",
  "src/tauto_project_store",
]

[tool.pytest.ini_options]
testpaths = ["tests"]
pythonpath = ["src"]

[tool.ruff]
line-length = 100
target-version = "py312"

[tool.ruff.lint]
select = ["E", "F", "I", "UP", "B"]
```

- [ ] **Step 2: Run metadata sanity check**

Run: `python -m pytest --version`

Expected: command prints a pytest version. If pytest is missing, install dev dependencies with `python -m pip install -e ".[dev]"`.

## Task 2: Package Import Smoke Test

**Files:**
- Create: `src/tauto_contract_ir/__init__.py`
- Create: `src/tauto_contract_parser/__init__.py`
- Create: `src/tauto_project_store/__init__.py`
- Create: `tests/test_imports.py`

- [ ] **Step 1: Write the failing import test**

```python
def test_tauto_packages_import() -> None:
    import tauto_contract_ir
    import tauto_contract_parser
    import tauto_project_store

    assert tauto_contract_ir.__all__ == []
    assert tauto_contract_parser.__all__ == []
    assert tauto_project_store.__all__ == []
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/test_imports.py -v`

Expected: FAIL with `ModuleNotFoundError` because packages do not exist yet.

- [ ] **Step 3: Create minimal packages**

`src/tauto_contract_ir/__init__.py`

```python
__all__: list[str] = []
```

`src/tauto_contract_parser/__init__.py`

```python
__all__: list[str] = []
```

`src/tauto_project_store/__init__.py`

```python
__all__: list[str] = []
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pytest tests/test_imports.py -v`

Expected: PASS.

## Task 3: Repository Documentation

**Files:**
- Create: `README.md`
- Create: `.gitignore`

- [ ] **Step 1: Create README**

````markdown
# Tauto

Tauto is a Lean-backed contract verification platform for business rules.

The first implementation slice focuses on a functional Python core:

- Markdown contract block extraction
- minimal contract DSL parsing
- language-independent Contract IR
- deterministic JSON and hashing

## Local Development

```bash
python -m pip install -e ".[dev]"
pytest
ruff check .
```
````

- [ ] **Step 2: Create `.gitignore`**

```gitignore
.pytest_cache/
.ruff_cache/
.venv/
__pycache__/
*.egg-info/
build/
dist/
```

- [ ] **Step 3: Verify docs do not affect tests**

Run: `pytest -q`

Expected: all tests pass.

## Self-Review Checklist

- [ ] No application behavior was implemented in Phase 0.
- [ ] Tests prove package imports only.
- [ ] Project uses `src/` layout and pytest can import from `src`.
- [ ] Package names are Tauto-specific and language-independent.
