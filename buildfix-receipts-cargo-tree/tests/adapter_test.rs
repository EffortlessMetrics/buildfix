use buildfix_adapter_sdk::Adapter;
use buildfix_adapter_sdk::AdapterTestHarness;
use buildfix_receipts_cargo_tree::CargoTreeAdapter;
use std::path::Path;

#[test]
fn test_adapter_loads_receipt() {
    let harness = AdapterTestHarness::new(CargoTreeAdapter::new());
    harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");
}

#[test]
fn test_adapter_finds_duplicates() {
    let adapter = CargoTreeAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    assert_eq!(receipt.findings.len(), 2);
    assert!(receipt.findings.iter().any(|f| {
        f.check_id
            .as_ref()
            .map(|c: &String| c.contains("duplicate"))
            .unwrap_or(false)
    }));
}

#[test]
fn test_adapter_schema() {
    let adapter = CargoTreeAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    assert_eq!(receipt.schema, "cargo-tree.duplicates.v1");
}
