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
