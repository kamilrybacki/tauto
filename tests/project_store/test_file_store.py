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
