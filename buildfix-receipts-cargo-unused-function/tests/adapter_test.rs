use buildfix_adapter_sdk::{Adapter, AdapterTestHarness};
use buildfix_receipts_cargo_unused_function::CargoUnusedFunctionAdapter;
use std::path::Path;

#[test]
fn test_adapter_loads_receipt() {
    let harness = AdapterTestHarness::new(CargoUnusedFunctionAdapter::new());
    harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");
}

#[test]
fn test_adapter_with_fixture_file() {
    let adapter = CargoUnusedFunctionAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    assert_eq!(receipt.findings.len(), 3);
    assert_eq!(receipt.verdict.counts.errors, 0);
    assert_eq!(receipt.verdict.counts.warnings, 3);
    assert_eq!(
        receipt.verdict.status,
        buildfix_types::receipt::VerdictStatus::Warn
    );
}

#[test]
fn test_adapter_finding_check_id() {
    let adapter = CargoUnusedFunctionAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    for finding in &receipt.findings {
        assert_eq!(
            finding.check_id,
            Some("dead_code.unused_function".to_string())
        );
        assert_eq!(finding.severity, buildfix_types::receipt::Severity::Warn);
    }
}
