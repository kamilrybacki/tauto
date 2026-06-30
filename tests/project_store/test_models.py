from tauto_project_store.models import Project


def test_project_normalizes_slug_and_defaults_contract_store_type() -> None:
    project = Project(name="Order Service", slug="Order Service")

    assert project.name == "Order Service"
    assert project.slug == "order-service"
    assert project.contract_store_type == "local"
