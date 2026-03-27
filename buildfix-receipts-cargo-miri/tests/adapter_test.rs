use buildfix_adapter_sdk::{Adapter, AdapterTestHarness};
use buildfix_receipts_cargo_miri::MiriAdapter;
use std::path::Path;

#[test]
fn test_adapter_loads_receipt() {
    let harness = AdapterTestHarness::new(MiriAdapter::new());
    harness
        .validate_receipt_fixture("tests/fixtures/report.jsonl")
        .expect("receipt should load correctly");
}

#[test]
fn test_adapter_with_fixture_file() {
    let adapter = MiriAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.jsonl"))
        .expect("should load fixture");

    assert_eq!(receipt.findings.len(), 3);
    assert_eq!(receipt.verdict.counts.errors, 3);
    assert_eq!(receipt.verdict.counts.warnings, 0);
}

#[test]
fn test_adapter_finding_count() {
    let adapter = MiriAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.jsonl"))
        .expect("should load fixture");

    let harness = AdapterTestHarness::new(MiriAdapter::new());
    harness
        .assert_finding_count(&receipt, 3, None)
        .expect("should have 3 findings");
}

#[test]
fn test_adapter_has_check_ids() {
    let adapter = MiriAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.jsonl"))
        .expect("should load fixture");

    let harness = AdapterTestHarness::new(MiriAdapter::new());
    harness
        .assert_has_check_id(&receipt, "miri.undefined_behavior")
        .expect("should have undefined_behavior check id");
}

#[test]
fn test_adapter_extracts_check_ids() {
    let adapter = MiriAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.jsonl"))
        .expect("should load fixture");

    let harness = AdapterTestHarness::new(MiriAdapter::new());
    let check_ids = harness.extract_check_ids(&receipt);

    assert!(check_ids.contains("miri.undefined_behavior"));
    assert!(check_ids.contains("miri.error"));
}
