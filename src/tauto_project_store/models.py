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
