use buildfix_adapter_sdk::{Adapter, AdapterTestHarness};
use buildfix_receipts_rustfmt::RustfmtAdapter;
use std::path::Path;

#[test]
fn test_adapter_loads_receipt() {
    let harness = AdapterTestHarness::new(RustfmtAdapter::new());
    harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");
}

#[test]
fn test_adapter_with_fixture_file() {
    let adapter = RustfmtAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    assert_eq!(receipt.findings.len(), 2);
    assert_eq!(receipt.verdict.counts.errors, 0);
    assert_eq!(receipt.verdict.counts.warnings, 2);
}

#[test]
fn test_adapter_has_check_ids() {
    let adapter = RustfmtAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    let harness = AdapterTestHarness::new(adapter);
    let check_ids = harness.extract_check_ids(&receipt);

    assert!(check_ids.contains("rustfmt.format"));
}

#[test]
fn test_adapter_finds_formatting_issue() {
    let adapter = RustfmtAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    let harness = AdapterTestHarness::new(adapter);
    harness
        .assert_has_check_id(&receipt, "rustfmt.format")
        .expect("should find rustfmt.format check id");
}
