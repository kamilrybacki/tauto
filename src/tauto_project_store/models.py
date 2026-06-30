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
