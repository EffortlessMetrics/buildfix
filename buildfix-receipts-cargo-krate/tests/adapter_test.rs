use buildfix_adapter_sdk::Adapter;
use buildfix_adapter_sdk::AdapterTestHarness;
use buildfix_receipts_cargo_krate::CargoKrateAdapter;
use std::path::Path;

#[test]
fn test_adapter_loads_receipt() {
    let harness = AdapterTestHarness::new(CargoKrateAdapter::new());
    harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");
}

#[test]
fn test_adapter_extracts_crates() {
    let adapter = CargoKrateAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    assert_eq!(receipt.findings.len(), 3);
    assert!(receipt.findings.iter().any(|f| {
        f.check_id
            .as_ref()
            .map(|c| c == "metadata.crate_info")
            .unwrap_or(false)
    }));
}

#[test]
fn test_adapter_finding_severity() {
    let adapter = CargoKrateAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    for finding in &receipt.findings {
        assert_eq!(finding.severity, buildfix_types::receipt::Severity::Info);
    }
}

#[test]
fn test_adapter_schema() {
    let adapter = CargoKrateAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    assert_eq!(receipt.schema, "cargo-krate.report.v1");
}

#[test]
fn test_adapter_status_pass() {
    let adapter = CargoKrateAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    assert_eq!(
        receipt.verdict.status,
        buildfix_types::receipt::VerdictStatus::Pass
    );
}
