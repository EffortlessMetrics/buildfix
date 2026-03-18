use buildfix_adapter_sdk::AdapterTestHarness;
use buildfix_receipts_cargo_warn::CargoWarnAdapter;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_adapter_loads_receipt() {
    let harness = AdapterTestHarness::new(CargoWarnAdapter::new());
    harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");
}

#[test]
fn test_adapter_with_empty_warnings() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");
    fs::write(
        &json_path,
        r#"{
            "warnings": []
        }"#,
    )
    .unwrap();

    let harness = AdapterTestHarness::new(CargoWarnAdapter::new());
    harness
        .validate_receipt_fixture(&json_path)
        .expect("receipt should load correctly");
}

#[test]
fn test_adapter_with_error_severity() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");
    fs::write(
        &json_path,
        r#"{
            "warnings": [
                {
                    "manifestPath": "/path/to/Cargo.toml",
                    "message": "critical error",
                    "code": "critical-error",
                    "severity": "error"
                }
            ]
        }"#,
    )
    .unwrap();

    let harness = AdapterTestHarness::new(CargoWarnAdapter::new());
    harness
        .validate_receipt_fixture(&json_path)
        .expect("receipt should load correctly");
}

#[test]
fn test_adapter_extracts_check_ids() {
    let adapter = CargoWarnAdapter::new();
    let harness = AdapterTestHarness::new(adapter);

    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load receipt");

    let check_ids = harness.extract_check_ids(&receipt);

    assert!(
        check_ids.contains("warn.unused_dependency"),
        "should have warn.unused_dependency"
    );
    assert!(
        check_ids.contains("warn.transitive_dependency"),
        "should have warn.transitive_dependency"
    );
}

#[test]
fn test_adapter_asserts_check_id() {
    let adapter = CargoWarnAdapter::new();
    let harness = AdapterTestHarness::new(adapter);

    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load receipt");

    harness
        .assert_has_check_id(&receipt, "warn.unused_dependency")
        .expect("should find check ID");
}
