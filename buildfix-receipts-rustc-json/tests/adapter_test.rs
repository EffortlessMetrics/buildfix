use buildfix_adapter_sdk::{Adapter, AdapterTestHarness};
use buildfix_receipts_rustc_json::RustcJsonAdapter;
use std::path::Path;

#[test]
fn test_adapter_loads_receipt() {
    let harness = AdapterTestHarness::new(RustcJsonAdapter::new());
    harness
        .validate_receipt_fixture("tests/fixtures/report.jsonl")
        .expect("receipt should load correctly");
}

#[test]
fn test_adapter_with_fixture_file() {
    let adapter = RustcJsonAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.jsonl"))
        .expect("should load fixture");

    assert_eq!(receipt.findings.len(), 5);
    assert_eq!(receipt.verdict.counts.errors, 2);
    assert_eq!(receipt.verdict.counts.warnings, 3);
}
