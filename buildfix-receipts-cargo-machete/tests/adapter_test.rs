use buildfix_adapter_sdk::AdapterTestHarness;
use buildfix_receipts_cargo_machete::CargoMacheteAdapter;

#[test]
fn test_adapter_loads_receipt_from_fixture() {
    let harness = AdapterTestHarness::new(CargoMacheteAdapter::new());
    harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");
}
