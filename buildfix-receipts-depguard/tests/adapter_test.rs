use buildfix_adapter_sdk::AdapterTestHarness;
use buildfix_receipts_depguard::DepguardAdapter;

#[test]
fn test_adapter_loads_receipt_from_fixture() {
    let harness = AdapterTestHarness::new(DepguardAdapter::new());
    harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");
}
