use buildfix_adapter_sdk::{Adapter, AdapterTestHarness};
use buildfix_receipts_cargo_crev::CargoCrevAdapter;
use std::path::Path;

#[test]
fn test_adapter_loads_receipt() {
    let harness = AdapterTestHarness::new(CargoCrevAdapter::new());
    harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");
}

#[test]
fn test_adapter_with_fixture_file() {
    let adapter = CargoCrevAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    assert_eq!(receipt.findings.len(), 3);
    assert_eq!(receipt.verdict.counts.errors, 1);
    assert_eq!(receipt.verdict.counts.warnings, 1);
    assert_eq!(
        receipt.verdict.status,
        buildfix_types::receipt::VerdictStatus::Fail
    );
}
