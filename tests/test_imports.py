def test_tauto_packages_import() -> None:
    import tauto_contract_ir
    import tauto_contract_parser
    import tauto_project_store

    assert tauto_contract_ir.__all__ == []
    assert tauto_contract_parser.__all__ == []
    assert tauto_project_store.__all__ == []
