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
            start_line = index
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
