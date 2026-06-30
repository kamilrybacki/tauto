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
