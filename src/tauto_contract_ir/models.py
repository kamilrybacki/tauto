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
