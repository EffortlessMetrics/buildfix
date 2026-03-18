use buildfix_adapter_sdk::Adapter;
use buildfix_adapter_sdk::AdapterTestHarness;
use buildfix_receipts_cargo_geiger::CargoGeigerAdapter;
use std::path::Path;

#[test]
fn test_adapter_loads_receipt() {
    let harness = AdapterTestHarness::new(CargoGeigerAdapter::new());
    harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");
}

#[test]
fn test_adapter_finds_unsafe_usage() {
    let adapter = CargoGeigerAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    assert_eq!(receipt.findings.len(), 3);
    assert!(receipt.findings.iter().any(|f| {
        f.check_id
            .as_ref()
            .map(|c| c.contains("safety.unsafe_usage"))
            .unwrap_or(false)
    }));
}

#[test]
fn test_adapter_schema() {
    let adapter = CargoGeigerAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    assert_eq!(receipt.schema, "cargo-geiger.report.v1");
}

#[test]
fn test_adapter_has_warnings() {
    let adapter = CargoGeigerAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    assert_eq!(receipt.verdict.counts.warnings, 3);
    assert_eq!(
        receipt.verdict.status,
        buildfix_types::receipt::VerdictStatus::Warn
    );
}
