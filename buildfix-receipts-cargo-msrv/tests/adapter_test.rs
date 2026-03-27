use buildfix_adapter_sdk::Adapter;
use buildfix_adapter_sdk::AdapterTestHarness;
use buildfix_receipts_cargo_msrv::CargoMsrvAdapter;

#[test]
fn test_adapter_fixture_loads() {
    let harness = AdapterTestHarness::new(CargoMsrvAdapter::new());
    harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");
}

#[test]
fn test_adapter_finds_incompatible_crates() {
    let harness = AdapterTestHarness::new(CargoMsrvAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load");

    let check_ids = harness.extract_check_ids(&receipt);
    assert!(
        check_ids.iter().any(|id| id.contains("msrv.incompatible")),
        "Should have msrv.incompatible findings"
    );
}

#[test]
fn test_adapter_warning_count() {
    let harness = AdapterTestHarness::new(CargoMsrvAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load");

    harness
        .assert_finding_count(&receipt, 2, None)
        .expect("assertion should pass");
    assert_eq!(receipt.verdict.counts.warnings, 2);
}

#[test]
fn test_adapter_has_correct_sensor_id() {
    let adapter = CargoMsrvAdapter::new();
    assert_eq!(adapter.sensor_id(), "cargo-msrv");
}
