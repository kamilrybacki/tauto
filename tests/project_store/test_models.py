from tauto_project_store.models import ContractDocument, Project


def test_project_normalizes_slug_and_defaults_contract_store_type() -> None:
    project = Project(name="Order Service", slug="Order Service")

    assert project.name == "Order Service"
    assert project.slug == "order-service"
    assert project.contract_store_type == "local"


def test_contract_document_keeps_markdown_and_version() -> None:
    document = ContractDocument(
        project_slug="order-service",
        path="business-cases/orders/cancel-paid-order.md",
        title="Cancel paid order",
        markdown_content="# Cancel paid order\n",
    )

    assert document.project_slug == "order-service"
    assert document.version == 1
    assert document.markdown_content == "# Cancel paid order\n"
