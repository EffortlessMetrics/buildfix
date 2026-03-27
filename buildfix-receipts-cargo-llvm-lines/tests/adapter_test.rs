use buildfix_adapter_sdk::Adapter;
use buildfix_adapter_sdk::AdapterTestHarness;
use buildfix_receipts_cargo_llvm_lines::CargoLlvmLinesAdapter;
use std::path::Path;

#[test]
fn test_adapter_loads_receipt() {
    let harness = AdapterTestHarness::new(CargoLlvmLinesAdapter::new());
    harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");
}

#[test]
fn test_adapter_finding_count() {
    let adapter = CargoLlvmLinesAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    assert_eq!(receipt.findings.len(), 2);
}

#[test]
fn test_adapter_has_check_ids() {
    let adapter = CargoLlvmLinesAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    let check_ids: Vec<&str> = receipt
        .findings
        .iter()
        .filter_map(|f| f.check_id.as_deref())
        .collect();

    assert!(
        check_ids.contains(&"llvm_lines.slow"),
        "should have llvm_lines.slow check_id"
    );
}

#[test]
fn test_adapter_schema() {
    let adapter = CargoLlvmLinesAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    assert_eq!(receipt.schema, "cargo-llvm-lines.report.v1");
}

#[test]
fn test_adapter_finding_messages() {
    let adapter = CargoLlvmLinesAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    assert!(receipt.findings.iter().any(|f| {
        f.message
            .as_ref()
            .unwrap()
            .contains("serde::ser::serialize")
    }));

    assert!(receipt.findings.iter().any(|f| {
        f.message
            .as_ref()
            .unwrap()
            .contains("tokio::runtime::block_on")
    }));
}

#[test]
fn test_adapter_locations() {
    let adapter = CargoLlvmLinesAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    assert!(
        receipt
            .findings
            .iter()
            .any(|f| f.location.as_ref().map(|l| l.path.as_str()) == Some("src/serialize.rs"))
    );

    assert!(
        receipt
            .findings
            .iter()
            .any(|f| f.location.as_ref().map(|l| l.path.as_str()) == Some("src/runtime.rs"))
    );
}
