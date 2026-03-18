use buildfix_adapter_sdk::AdapterTestHarness;
use buildfix_receipts_cargo_cyclonedds::CargoHackAdapter;

#[test]
fn test_adapter_loads_receipt() {
    let harness = AdapterTestHarness::new(CargoHackAdapter::new());
    harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");
}

#[test]
fn test_adapter_finding_count() {
    let harness = AdapterTestHarness::new(CargoHackAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load");
    harness
        .assert_finding_count(&receipt, 3, None)
        .expect("should have 3 findings");
}

#[test]
fn test_adapter_has_check_id() {
    let harness = AdapterTestHarness::new(CargoHackAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load");
    harness
        .assert_has_check_id(&receipt, "hack.unstable")
        .expect("should have hack.unstable check id");
}

#[test]
fn test_adapter_extracts_check_ids() {
    let harness = AdapterTestHarness::new(CargoHackAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load");
    let check_ids = harness.extract_check_ids(&receipt);

    assert!(
        check_ids.iter().all(|id| id == "hack.unstable"),
        "all check IDs should be hack.unstable"
    );
}

#[test]
fn test_adapter_golden_test() {
    let harness = AdapterTestHarness::new(CargoHackAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load");
    harness
        .golden_test("tests/fixtures/report.json", &receipt)
        .expect("golden test should pass");
}
