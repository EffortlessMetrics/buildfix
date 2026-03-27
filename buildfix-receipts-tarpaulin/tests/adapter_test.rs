use buildfix_adapter_sdk::AdapterTestHarness;
use buildfix_receipts_tarpaulin::TarpaulinAdapter;

#[test]
fn test_adapter_loads_receipt() {
    let harness = AdapterTestHarness::new(TarpaulinAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");

    let validation = harness.validate_receipt(&receipt);
    assert!(validation.is_valid());
}

#[test]
fn test_adapter_finding_count() {
    let harness = AdapterTestHarness::new(TarpaulinAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load");

    harness
        .assert_finding_count(&receipt, 4, None)
        .expect("should have 4 findings (1 overall + 3 files)");
}

#[test]
fn test_adapter_has_check_id() {
    let harness = AdapterTestHarness::new(TarpaulinAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load");

    harness
        .assert_has_check_id(&receipt, "tarpaulin.low_coverage")
        .expect("should have tarpaulin.low_coverage check id");
}

#[test]
fn test_adapter_extracts_check_ids() {
    let harness = AdapterTestHarness::new(TarpaulinAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load");

    let check_ids = harness.extract_check_ids(&receipt);

    assert!(
        check_ids.iter().all(|id| id == "tarpaulin.low_coverage"),
        "all check_ids should be tarpaulin.low_coverage"
    );
}
