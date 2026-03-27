use buildfix_adapter_sdk::AdapterTestHarness;
use buildfix_receipts_cargo_lock::CargoLockAdapter;

#[test]
fn test_adapter_loads_receipt_fixture() {
    let harness = AdapterTestHarness::new(CargoLockAdapter::new());
    harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");
}

#[test]
fn test_adapter_has_correct_check_id() {
    let harness = AdapterTestHarness::new(CargoLockAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load fixture");

    harness
        .assert_has_check_id(&receipt, "lock.warnings")
        .expect("should have lock.warnings check id");
}

#[test]
fn test_adapter_extracts_check_ids() {
    let harness = AdapterTestHarness::new(CargoLockAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load fixture");

    let check_ids = harness.extract_check_ids(&receipt);
    assert!(check_ids.contains("lock.warnings"));
}

#[test]
fn test_adapter_finding_count() {
    let harness = AdapterTestHarness::new(CargoLockAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load fixture");

    harness
        .assert_finding_count(&receipt, 1, None)
        .expect("should have 1 finding");
}
