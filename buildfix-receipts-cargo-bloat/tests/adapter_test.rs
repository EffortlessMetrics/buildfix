use buildfix_adapter_sdk::Adapter;
use buildfix_adapter_sdk::AdapterTestHarness;
use buildfix_receipts_cargo_bloat::CargoBloatAdapter;
use std::path::Path;

#[test]
fn test_adapter_loads_receipt() {
    let harness = AdapterTestHarness::new(CargoBloatAdapter::new());
    harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");
}

#[test]
fn test_adapter_finding_count() {
    let adapter = CargoBloatAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    assert_eq!(receipt.findings.len(), 2);
}

#[test]
fn test_adapter_has_check_ids() {
    let adapter = CargoBloatAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    let check_ids: Vec<&str> = receipt
        .findings
        .iter()
        .filter_map(|f| f.check_id.as_deref())
        .collect();

    assert!(
        check_ids.contains(&"size.large_crate"),
        "should have size.large_crate check_id"
    );
}

#[test]
fn test_adapter_schema() {
    let adapter = CargoBloatAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    assert_eq!(receipt.schema, "cargo-bloat.report.v1");
}
