use buildfix_adapter_sdk::AdapterTestHarness;
use buildfix_receipts_cargo_update::CargoUpdateAdapter;

#[test]
fn test_adapter_loads_receipt_fixture() {
    let harness = AdapterTestHarness::new(CargoUpdateAdapter::new());
    harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");
}

#[test]
fn test_adapter_has_correct_sensor_id() {
    use buildfix_adapter_sdk::Adapter;
    let adapter = CargoUpdateAdapter::new();
    assert_eq!(adapter.sensor_id(), "cargo-update");
}

#[test]
fn test_adapter_finding_count() {
    let harness = AdapterTestHarness::new(CargoUpdateAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load receipt");
    harness.assert_finding_count(&receipt, 3, None).unwrap();
}

#[test]
fn test_adapter_has_check_id() {
    let harness = AdapterTestHarness::new(CargoUpdateAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load receipt");
    harness
        .assert_has_check_id(&receipt, "update.available")
        .unwrap();
}

#[test]
fn test_adapter_extracts_check_ids() {
    let harness = AdapterTestHarness::new(CargoUpdateAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load receipt");
    let check_ids = harness.extract_check_ids(&receipt);
    assert_eq!(check_ids.len(), 1);
    assert!(check_ids.contains("update.available"));
}

#[test]
fn test_adapter_golden_test() {
    let harness = AdapterTestHarness::new(CargoUpdateAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load receipt");
    harness
        .golden_test("tests/fixtures/report.json", &receipt)
        .expect("golden test should pass");
}
