# Tauto Phase 2 Contract Parser And IR Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Parse Markdown contract blocks into deterministic, language-independent Contract IR and ContractSet JSON.

**Architecture:** The parser is a functional core: strings in, typed values plus diagnostics out. Contract IR is independent of Lean, Python, or TypeScript; backend generators will consume it in later phases.

**Tech Stack:** Python 3.12, Pydantic v2, pytest, hashlib, json.

---

## File Structure

- Create: `src/tauto_contract_ir/models.py` - expression, condition, operation call, contract, contract set, diagnostics, source locations.
- Create: `src/tauto_contract_ir/serialization.py` - canonical JSON and SHA-256 hashes.
- Modify: `src/tauto_contract_ir/__init__.py` - public exports.
- Create: `src/tauto_contract_parser/markdown.py` - fenced `contract` block extraction.
- Create: `src/tauto_contract_parser/dsl.py` - minimal DSL parser.
- Modify: `src/tauto_contract_parser/__init__.py` - public exports.
- Create: `tests/contract_ir/test_models.py`
- Create: `tests/contract_ir/test_serialization.py`
- Create: `tests/contract_parser/test_markdown.py`
- Create: `tests/contract_parser/test_dsl.py`

## Task 1: IR Models For Zero Contract Set

**Files:**
- Create: `src/tauto_contract_ir/models.py`
- Test: `tests/contract_ir/test_models.py`

- [ ] **Step 1: Write failing zero-contract test**

```python
from tauto_contract_ir.models import ContractSet


def test_zero_contract_set_is_valid() -> None:
    contract_set = ContractSet()

    assert contract_set.schema_version == 1
    assert contract_set.contracts == []
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/contract_ir/test_models.py::test_zero_contract_set_is_valid -v`

Expected: FAIL because `tauto_contract_ir.models` does not exist.

- [ ] **Step 3: Implement minimal ContractSet**

```python
from pydantic import BaseModel, ConfigDict, Field


class ContractSet(BaseModel):
    model_config = ConfigDict(frozen=True)

    schema_version: int = 1
    contracts: list["ContractIR"] = Field(default_factory=list)


class ContractIR(BaseModel):
    model_config = ConfigDict(frozen=True)

    schema_version: int = 1
    case: str
    entity: str
    operation: str
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pytest tests/contract_ir/test_models.py::test_zero_contract_set_is_valid -v`

Expected: PASS.

## Task 2: Full IR Model Shape

**Files:**
- Modify: `src/tauto_contract_ir/models.py`
- Test: `tests/contract_ir/test_models.py`

- [ ] **Step 1: Write failing model-shape test**

```python
from tauto_contract_ir.models import (
    Condition,
    ContractIR,
    Expression,
    ForbiddenOperation,
    SourceLocation,
)


def test_contract_ir_holds_language_independent_contract_parts() -> None:
    contract = ContractIR(
        case="CancelPaidOrder",
        entity="Order",
        operation="cancelOrder",
        requires=[
            Condition(
                left=Expression(kind="field", value="order.status"),
                operator="==",
                right=Expression(kind="enum", value="Paid"),
            )
        ],
        ensures=[
            Condition(
                left=Expression(kind="field", value="result.total"),
                operator="==",
                right=Expression(kind="field", value="order.total"),
            )
        ],
        forbidden=[
            ForbiddenOperation(
                operation="shipOrder",
                args=[Expression(kind="variable", value="result")],
            )
        ],
        preserves=["ValidOrder"],
        assumes=[],
        source=SourceLocation(
            document_path="business-cases/orders/cancel-paid-order.md",
            start_line=3,
            end_line=22,
        ),
    )

    assert contract.requires[0].left.kind == "field"
    assert contract.forbidden[0].operation == "shipOrder"
    assert contract.source.start_line == 3
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/contract_ir/test_models.py::test_contract_ir_holds_language_independent_contract_parts -v`

Expected: FAIL because supporting models or fields are missing.

- [ ] **Step 3: Implement IR models**

```python
from typing import Literal

from pydantic import BaseModel, ConfigDict, Field


ExpressionKind = Literal["field", "variable", "enum", "int", "bool"]
ComparisonOperator = Literal["==", "!=", ">=", "<=", ">", "<"]


class Expression(BaseModel):
    model_config = ConfigDict(frozen=True)

    kind: ExpressionKind
    value: str | int | bool


class Condition(BaseModel):
    model_config = ConfigDict(frozen=True)

    left: Expression
    operator: ComparisonOperator
    right: Expression


class ForbiddenOperation(BaseModel):
    model_config = ConfigDict(frozen=True)

    operation: str
    args: list[Expression] = Field(default_factory=list)


class SourceLocation(BaseModel):
    model_config = ConfigDict(frozen=True)

    document_path: str
    start_line: int
    end_line: int


class Diagnostic(BaseModel):
    model_config = ConfigDict(frozen=True)

    category: str
    message: str
    document_path: str | None = None
    line: int | None = None
    suggestion: str | None = None


class ContractIR(BaseModel):
    model_config = ConfigDict(frozen=True)

    schema_version: int = 1
    case: str
    entity: str
    operation: str
    requires: list[Condition] = Field(default_factory=list)
    ensures: list[Condition] = Field(default_factory=list)
    forbidden: list[ForbiddenOperation] = Field(default_factory=list)
    preserves: list[str] = Field(default_factory=list)
    assumes: list[str] = Field(default_factory=list)
    source: SourceLocation | None = None


class ContractSet(BaseModel):
    model_config = ConfigDict(frozen=True)

    schema_version: int = 1
    contracts: list[ContractIR] = Field(default_factory=list)
```

- [ ] **Step 4: Run IR model tests**

Run: `pytest tests/contract_ir/test_models.py -v`

Expected: PASS.

## Task 3: Deterministic Serialization And Hashing

**Files:**
- Create: `src/tauto_contract_ir/serialization.py`
- Test: `tests/contract_ir/test_serialization.py`

- [ ] **Step 1: Write failing serialization tests**

```python
from tauto_contract_ir.models import ContractIR, ContractSet
from tauto_contract_ir.serialization import canonical_contract_set_json, contract_set_hash


def test_zero_contract_set_canonical_json_is_stable() -> None:
    assert canonical_contract_set_json(ContractSet()) == (
        '{"contracts":[],"schema_version":1}'
    )


def test_contract_set_hash_changes_when_semantic_content_changes() -> None:
    first = ContractSet(contracts=[ContractIR(case="A", entity="Order", operation="cancelOrder")])
    second = ContractSet(contracts=[ContractIR(case="B", entity="Order", operation="cancelOrder")])

    assert contract_set_hash(first) != contract_set_hash(second)
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `pytest tests/contract_ir/test_serialization.py -v`

Expected: FAIL because serialization module does not exist.

- [ ] **Step 3: Implement canonical serialization**

```python
import hashlib
import json

from tauto_contract_ir.models import ContractIR, ContractSet


def canonical_contract_json(contract: ContractIR) -> str:
    return json.dumps(
        contract.model_dump(exclude_none=True),
        sort_keys=True,
        separators=(",", ":"),
    )


def canonical_contract_set_json(contract_set: ContractSet) -> str:
    return json.dumps(
        contract_set.model_dump(exclude_none=True),
        sort_keys=True,
        separators=(",", ":"),
    )


def contract_hash(contract: ContractIR) -> str:
    return hashlib.sha256(canonical_contract_json(contract).encode("utf-8")).hexdigest()


def contract_set_hash(contract_set: ContractSet) -> str:
    return hashlib.sha256(
        canonical_contract_set_json(contract_set).encode("utf-8")
    ).hexdigest()
```

- [ ] **Step 4: Run serialization tests**

Run: `pytest tests/contract_ir/test_serialization.py -v`

Expected: PASS.

## Task 4: Markdown Contract Block Extraction

**Files:**
- Create: `src/tauto_contract_parser/markdown.py`
- Test: `tests/contract_parser/test_markdown.py`

- [ ] **Step 1: Write failing extraction tests**

````python
from tauto_contract_parser.markdown import extract_contract_blocks


def test_empty_markdown_has_no_contract_blocks() -> None:
    assert extract_contract_blocks("# Rules\n", "rules.md") == []


def test_extracts_contract_block_with_source_lines() -> None:
    markdown = """# Cancel paid order

Intro text.

```contract
case CancelPaidOrder

entity:
  Order
```
"""

    blocks = extract_contract_blocks(markdown, "rules.md")

    assert len(blocks) == 1
    assert blocks[0].raw_block == "case CancelPaidOrder\n\nentity:\n  Order"
    assert blocks[0].source.document_path == "rules.md"
    assert blocks[0].source.start_line == 5
    assert blocks[0].source.end_line == 9
```
````

- [ ] **Step 2: Run tests to verify they fail**

Run: `pytest tests/contract_parser/test_markdown.py -v`

Expected: FAIL because extraction module does not exist.

- [ ] **Step 3: Implement extraction**

```python
from pydantic import BaseModel, ConfigDict

from tauto_contract_ir.models import SourceLocation


class ContractBlock(BaseModel):
    model_config = ConfigDict(frozen=True)

    raw_block: str
    source: SourceLocation


def extract_contract_blocks(markdown: str, document_path: str) -> list[ContractBlock]:
    lines = markdown.splitlines()
    blocks: list[ContractBlock] = []
    in_block = False
    start_line = 0
    block_lines: list[str] = []

    for index, line in enumerate(lines, start=1):
        if not in_block and line.strip() == "```contract":
            in_block = True
            start_line = index + 1
            block_lines = []
            continue

        if in_block and line.strip() == "```":
            blocks.append(
                ContractBlock(
                    raw_block="\n".join(block_lines),
                    source=SourceLocation(
                        document_path=document_path,
                        start_line=start_line,
                        end_line=index - 1,
                    ),
                )
            )
            in_block = False
            continue

        if in_block:
            block_lines.append(line)

    return blocks
```

- [ ] **Step 4: Run extraction tests**

Run: `pytest tests/contract_parser/test_markdown.py -v`

Expected: PASS.

## Task 5: DSL Parser For CancelPaidOrder

**Files:**
- Create: `src/tauto_contract_parser/dsl.py`
- Test: `tests/contract_parser/test_dsl.py`

- [ ] **Step 1: Write failing parser test**

```python
from tauto_contract_ir.models import SourceLocation
from tauto_contract_parser.dsl import parse_contract_block
from tauto_contract_parser.markdown import ContractBlock


def test_parse_cancel_paid_order_contract() -> None:
    block = ContractBlock(
        raw_block="""case CancelPaidOrder

entity:
  Order

operation:
  cancelOrder

requires:
  order.status == Paid
  order.shipped == false

ensures:
  result.status == Cancelled
  result.total == order.total
  result.id == order.id

forbidden:
  shipOrder(result)

preserves:
  ValidOrder
  CancelledOrderCannotShip
""",
        source=SourceLocation(document_path="rules.md", start_line=1, end_line=24),
    )

    parsed = parse_contract_block(block)

    assert parsed.diagnostics == []
    assert parsed.contract is not None
    assert parsed.contract.case == "CancelPaidOrder"
    assert parsed.contract.entity == "Order"
    assert parsed.contract.operation == "cancelOrder"
    assert parsed.contract.requires[1].right.kind == "bool"
    assert parsed.contract.requires[1].right.value is False
    assert parsed.contract.ensures[1].right.kind == "field"
    assert parsed.contract.forbidden[0].operation == "shipOrder"
    assert parsed.contract.preserves == ["ValidOrder", "CancelledOrderCannotShip"]
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/contract_parser/test_dsl.py::test_parse_cancel_paid_order_contract -v`

Expected: FAIL because DSL parser does not exist.

- [ ] **Step 3: Implement parser result and parser**

```python
import re
from typing import Literal

from pydantic import BaseModel, ConfigDict

from tauto_contract_ir.models import (
    Condition,
    ContractIR,
    Diagnostic,
    Expression,
    ForbiddenOperation,
)
from tauto_contract_parser.markdown import ContractBlock


SectionName = Literal[
    "entity",
    "operation",
    "requires",
    "ensures",
    "forbidden",
    "preserves",
    "assumes",
]

CONDITION_RE = re.compile(r"^(.+?)\s*(==|!=|>=|<=|>|<)\s*(.+)$")
CALL_RE = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*)\((.*)\)$")


class ParseResult(BaseModel):
    model_config = ConfigDict(frozen=True)

    contract: ContractIR | None
    diagnostics: list[Diagnostic]


def parse_contract_block(block: ContractBlock) -> ParseResult:
    case_name: str | None = None
    sections: dict[str, list[tuple[int, str]]] = {}
    current_section: str | None = None
    diagnostics: list[Diagnostic] = []

    for offset, raw_line in enumerate(block.raw_block.splitlines()):
        line_number = block.source.start_line + offset
        stripped = raw_line.strip()
        if stripped == "":
            continue
        if stripped.startswith("case "):
            case_name = stripped.removeprefix("case ").strip()
            continue
        if stripped.endswith(":"):
            current_section = stripped.removesuffix(":")
            sections.setdefault(current_section, [])
            continue
        if current_section is None:
            diagnostics.append(
                Diagnostic(
                    category="parse_error",
                    message=f"Line is outside a section: {stripped}",
                    document_path=block.source.document_path,
                    line=line_number,
                )
            )
            continue
        sections.setdefault(current_section, []).append((line_number, stripped))

    entity = _single_value("entity", sections, block, diagnostics)
    operation = _single_value("operation", sections, block, diagnostics)

    if case_name is None:
        diagnostics.append(
            Diagnostic(
                category="parse_error",
                message="Missing case declaration",
                document_path=block.source.document_path,
                line=block.source.start_line,
            )
        )

    requires = [_parse_condition(item, block, diagnostics) for item in sections.get("requires", [])]
    ensures = [_parse_condition(item, block, diagnostics) for item in sections.get("ensures", [])]
    forbidden = [_parse_forbidden(item, block, diagnostics) for item in sections.get("forbidden", [])]

    if diagnostics or case_name is None or entity is None or operation is None:
        return ParseResult(contract=None, diagnostics=diagnostics)

    return ParseResult(
        contract=ContractIR(
            case=case_name,
            entity=entity,
            operation=operation,
            requires=[condition for condition in requires if condition is not None],
            ensures=[condition for condition in ensures if condition is not None],
            forbidden=[call for call in forbidden if call is not None],
            preserves=[value for _, value in sections.get("preserves", [])],
            assumes=[value for _, value in sections.get("assumes", [])],
            source=block.source,
        ),
        diagnostics=[],
    )


def _single_value(
    section: str,
    sections: dict[str, list[tuple[int, str]]],
    block: ContractBlock,
    diagnostics: list[Diagnostic],
) -> str | None:
    values = sections.get(section, [])
    if len(values) == 1:
        return values[0][1]
    diagnostics.append(
        Diagnostic(
            category="parse_error",
            message=f"Section '{section}' must contain exactly one value",
            document_path=block.source.document_path,
            line=block.source.start_line,
        )
    )
    return None


def _parse_condition(
    item: tuple[int, str],
    block: ContractBlock,
    diagnostics: list[Diagnostic],
) -> Condition | None:
    line, text = item
    match = CONDITION_RE.match(text)
    if match is None:
        diagnostics.append(
            Diagnostic(
                category="parse_error",
                message=f"Malformed condition: {text}",
                document_path=block.source.document_path,
                line=line,
            )
        )
        return None
    left, operator, right = match.groups()
    return Condition(
        left=_parse_expression(left.strip()),
        operator=operator,  # type: ignore[arg-type]
        right=_parse_expression(right.strip()),
    )


def _parse_forbidden(
    item: tuple[int, str],
    block: ContractBlock,
    diagnostics: list[Diagnostic],
) -> ForbiddenOperation | None:
    line, text = item
    match = CALL_RE.match(text)
    if match is None:
        diagnostics.append(
            Diagnostic(
                category="parse_error",
                message=f"Malformed forbidden operation call: {text}",
                document_path=block.source.document_path,
                line=line,
            )
        )
        return None
    operation, raw_args = match.groups()
    args = [] if raw_args.strip() == "" else [_parse_expression(arg.strip()) for arg in raw_args.split(",")]
    return ForbiddenOperation(operation=operation, args=args)


def _parse_expression(raw: str) -> Expression:
    if raw == "true":
        return Expression(kind="bool", value=True)
    if raw == "false":
        return Expression(kind="bool", value=False)
    if raw.removeprefix("-").isdigit():
        return Expression(kind="int", value=int(raw))
    if "." in raw:
        return Expression(kind="field", value=raw)
    if raw[:1].islower():
        return Expression(kind="variable", value=raw)
    return Expression(kind="enum", value=raw)
```

- [ ] **Step 4: Run parser test**

Run: `pytest tests/contract_parser/test_dsl.py::test_parse_cancel_paid_order_contract -v`

Expected: PASS.

## Task 6: Parser Diagnostics For Malformed Conditions

**Files:**
- Modify: `src/tauto_contract_parser/dsl.py`
- Test: `tests/contract_parser/test_dsl.py`

- [ ] **Step 1: Write failing diagnostics test**

```python
from tauto_contract_ir.models import SourceLocation
from tauto_contract_parser.dsl import parse_contract_block
from tauto_contract_parser.markdown import ContractBlock


def test_parse_reports_malformed_condition_with_source_line() -> None:
    block = ContractBlock(
        raw_block="""case CancelPaidOrder
entity:
  Order
operation:
  cancelOrder
requires:
  order.status Paid
""",
        source=SourceLocation(document_path="rules.md", start_line=10, end_line=16),
    )

    parsed = parse_contract_block(block)

    assert parsed.contract is None
    assert parsed.diagnostics[0].category == "parse_error"
    assert parsed.diagnostics[0].line == 16
    assert "Malformed condition" in parsed.diagnostics[0].message
```

- [ ] **Step 2: Run test to verify it fails if diagnostics are incomplete**

Run: `pytest tests/contract_parser/test_dsl.py::test_parse_reports_malformed_condition_with_source_line -v`

Expected: FAIL if malformed conditions are ignored or line numbers are wrong.

- [ ] **Step 3: Ensure parser returns no contract when any diagnostic exists**

In `parse_contract_block`, keep this guard after parsing conditions and forbidden operations:

```python
if diagnostics or case_name is None or entity is None or operation is None:
    return ParseResult(contract=None, diagnostics=diagnostics)
```

- [ ] **Step 4: Run DSL parser tests**

Run: `pytest tests/contract_parser/test_dsl.py -v`

Expected: PASS.

## Task 7: Public Exports

**Files:**
- Modify: `src/tauto_contract_ir/__init__.py`
- Modify: `src/tauto_contract_parser/__init__.py`
- Test: `tests/test_imports.py`

- [ ] **Step 1: Replace import smoke test with public API test**

```python
from tauto_contract_ir import ContractIR, ContractSet
from tauto_contract_parser import extract_contract_blocks, parse_contract_block


def test_tauto_packages_export_core_symbols() -> None:
    assert ContractSet().contracts == []
    assert ContractIR(case="A", entity="Order", operation="cancelOrder").case == "A"
    assert extract_contract_blocks("# Empty\n", "rules.md") == []
    assert callable(parse_contract_block)
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/test_imports.py -v`

Expected: FAIL because public exports are not wired.

- [ ] **Step 3: Export IR symbols**

`src/tauto_contract_ir/__init__.py`

```python
from tauto_contract_ir.models import (
    Condition,
    ContractIR,
    ContractSet,
    Diagnostic,
    Expression,
    ForbiddenOperation,
    SourceLocation,
)
from tauto_contract_ir.serialization import (
    canonical_contract_json,
    canonical_contract_set_json,
    contract_hash,
    contract_set_hash,
)

__all__ = [
    "Condition",
    "ContractIR",
    "ContractSet",
    "Diagnostic",
    "Expression",
    "ForbiddenOperation",
    "SourceLocation",
    "canonical_contract_json",
    "canonical_contract_set_json",
    "contract_hash",
    "contract_set_hash",
]
```

`src/tauto_contract_parser/__init__.py`

```python
from tauto_contract_parser.dsl import ParseResult, parse_contract_block
from tauto_contract_parser.markdown import ContractBlock, extract_contract_blocks

__all__ = [
    "ContractBlock",
    "ParseResult",
    "extract_contract_blocks",
    "parse_contract_block",
]
```

- [ ] **Step 4: Run full test suite**

Run: `pytest -q`

Expected: PASS.

## Self-Review Checklist

- [ ] The core IR contains no Lean-specific types or names.
- [ ] `ContractSet` supports zero contracts.
- [ ] Parser functions return structured diagnostics instead of raising on normal user errors.
- [ ] Serialization uses sorted keys and compact separators.
- [ ] Tests cover extraction, parsing, malformed input, deterministic JSON, and hash changes.
