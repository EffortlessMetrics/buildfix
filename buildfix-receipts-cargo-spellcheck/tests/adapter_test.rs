use buildfix_adapter_sdk::Adapter;
use buildfix_adapter_sdk::AdapterTestHarness;
use std::path::Path;

use buildfix_receipts_cargo_spellcheck::CargoSpellcheckAdapter;

#[test]
fn test_adapter_loads_fixture() {
    let harness = AdapterTestHarness::new(CargoSpellcheckAdapter::new());
    harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");
}

#[test]
fn test_adapter_loads_file() {
    let adapter = CargoSpellcheckAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    assert_eq!(receipt.findings.len(), 4);
    assert_eq!(receipt.verdict.counts.findings, 4);
}

#[test]
fn test_adapter_sensor_id() {
    let adapter = CargoSpellcheckAdapter::new();
    assert_eq!(adapter.sensor_id(), "cargo-spellcheck");
}

#[test]
fn test_adapter_check_ids() {
    let adapter = CargoSpellcheckAdapter::new();
    let receipt = adapter
        .load(Path::new("tests/fixtures/report.json"))
        .expect("should load receipt");

    let harness = AdapterTestHarness::new(CargoSpellcheckAdapter::new());
    let check_ids = harness.extract_check_ids(&receipt);

    assert!(check_ids.contains("docs.spelling_error"));
    assert!(check_ids.contains("spellcheck.spelling"));
}
