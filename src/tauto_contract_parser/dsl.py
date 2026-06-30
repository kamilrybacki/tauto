import re
from typing import Literal, cast

from pydantic import BaseModel, ConfigDict

from tauto_contract_ir.models import (
    ComparisonOperator,
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
        operator=cast(ComparisonOperator, operator),
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
    args = (
        []
        if raw_args.strip() == ""
        else [_parse_expression(arg.strip()) for arg in raw_args.split(",")]
    )
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
