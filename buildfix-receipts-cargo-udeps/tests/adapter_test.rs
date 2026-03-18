use buildfix_adapter_sdk::AdapterTestHarness;
use buildfix_receipts_cargo_udeps::CargoUdepsAdapter;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_adapter_loads_receipt() {
    let harness = AdapterTestHarness::new(CargoUdepsAdapter::new());
    harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");
}

#[test]
fn test_adapter_with_empty_packages() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");
    fs::write(
        &json_path,
        r#"{
            "success": true,
            "packages": []
        }"#,
    )
    .unwrap();

    let harness = AdapterTestHarness::new(CargoUdepsAdapter::new());
    harness
        .validate_receipt_fixture(&json_path)
        .expect("receipt should load correctly");
}

#[test]
fn test_adapter_with_no_packages_field() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");
    fs::write(&json_path, r#"{"success": true}"#).unwrap();

    let harness = AdapterTestHarness::new(CargoUdepsAdapter::new());
    harness
        .validate_receipt_fixture(&json_path)
        .expect("receipt should load correctly");
}
