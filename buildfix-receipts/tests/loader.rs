//! Unit tests for receipt loader.

use buildfix_receipts::{ReceiptLoadError, load_receipts};
use camino::Utf8PathBuf;
use std::fs;
use tempfile::TempDir;

fn create_temp_dir() -> TempDir {
    tempfile::tempdir().expect("tempdir")
}

fn artifacts_path(temp: &TempDir) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(temp.path().join("artifacts")).unwrap()
}

fn create_receipt(dir: &Utf8PathBuf, sensor: &str, contents: &str) {
    let sensor_dir = dir.join(sensor);
    fs::create_dir_all(&sensor_dir).unwrap();
    fs::write(sensor_dir.join("report.json"), contents).unwrap();
}

fn valid_receipt() -> &'static str {
    r#"{
        "schema": "test.report.v1",
        "tool": { "name": "test-sensor", "version": "1.0.0" },
        "verdict": { "status": "pass", "counts": { "findings": 0, "errors": 0, "warnings": 0 } },
        "findings": []
    }"#
}

#[test]
fn test_empty_artifacts_dir() {
    let temp = create_temp_dir();
    let artifacts = artifacts_path(&temp);
    fs::create_dir_all(&artifacts).unwrap();

    let receipts = load_receipts(&artifacts).unwrap();
    assert!(receipts.is_empty());
}

#[test]
fn test_missing_artifacts_dir() {
    let temp = create_temp_dir();
    let artifacts = artifacts_path(&temp);
    // Don't create the directory

    let receipts = load_receipts(&artifacts).unwrap();
    assert!(receipts.is_empty());
}

#[test]
fn test_single_valid_receipt() {
    let temp = create_temp_dir();
    let artifacts = artifacts_path(&temp);
    create_receipt(&artifacts, "builddiag", valid_receipt());

    let receipts = load_receipts(&artifacts).unwrap();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].sensor_id, "builddiag");
    assert!(receipts[0].receipt.is_ok());
}

#[test]
fn test_multiple_receipts_sorted_deterministically() {
    let temp = create_temp_dir();
    let artifacts = artifacts_path(&temp);

    // Create in non-alphabetical order
    create_receipt(&artifacts, "zebra", valid_receipt());
    create_receipt(&artifacts, "alpha", valid_receipt());
    create_receipt(&artifacts, "middle", valid_receipt());

    let receipts = load_receipts(&artifacts).unwrap();
    assert_eq!(receipts.len(), 3);

    // Should be sorted by path
    assert_eq!(receipts[0].sensor_id, "alpha");
    assert_eq!(receipts[1].sensor_id, "middle");
    assert_eq!(receipts[2].sensor_id, "zebra");
}

#[test]
fn test_corrupted_json_collected_without_failing() {
    let temp = create_temp_dir();
    let artifacts = artifacts_path(&temp);

    create_receipt(&artifacts, "good", valid_receipt());
    create_receipt(&artifacts, "bad", "{ not valid json }}}");

    let receipts = load_receipts(&artifacts).unwrap();
    assert_eq!(receipts.len(), 2);

    // Should still load both, one with error
    let good = receipts.iter().find(|r| r.sensor_id == "good").unwrap();
    let bad = receipts.iter().find(|r| r.sensor_id == "bad").unwrap();

    assert!(good.receipt.is_ok());
    assert!(matches!(bad.receipt, Err(ReceiptLoadError::Json { .. })));
}

#[test]
fn test_missing_schema_field() {
    let temp = create_temp_dir();
    let artifacts = artifacts_path(&temp);

    // Valid JSON but missing required "schema" field
    let incomplete = r#"{
        "tool": { "name": "incomplete", "version": "0.0.0" },
        "verdict": { "status": "pass", "counts": { "findings": 0, "errors": 0, "warnings": 0 } },
        "findings": []
    }"#;

    create_receipt(&artifacts, "incomplete", incomplete);

    let receipts = load_receipts(&artifacts).unwrap();
    assert_eq!(receipts.len(), 1);

    // Should fail to parse due to missing schema
    assert!(matches!(
        receipts[0].receipt,
        Err(ReceiptLoadError::Json { .. })
    ));
}

#[test]
fn test_report_json_directory_yields_io_error() {
    let temp = create_temp_dir();
    let artifacts = artifacts_path(&temp);

    let sensor_dir = artifacts.join("weird");
    fs::create_dir_all(&sensor_dir).unwrap();
    // Create report.json as a directory to force an IO error on read.
    fs::create_dir_all(sensor_dir.join("report.json")).unwrap();

    let receipts = load_receipts(&artifacts).unwrap();
    assert_eq!(receipts.len(), 1);
    assert!(matches!(
        receipts[0].receipt,
        Err(ReceiptLoadError::Io { .. })
    ));
}

#[test]
fn test_extra_fields_tolerated() {
    let temp = create_temp_dir();
    let artifacts = artifacts_path(&temp);

    // Valid JSON with extra fields that should be ignored
    let with_extras = r#"{
        "schema": "test.report.v1",
        "tool": { "name": "test", "version": "1.0.0", "extra_field": "ignored" },
        "verdict": { "status": "pass", "counts": { "findings": 0, "errors": 0, "warnings": 0 } },
        "findings": [],
        "custom_data": { "anything": "goes" },
        "another_unknown": [1, 2, 3]
    }"#;

    create_receipt(&artifacts, "flexible", with_extras);

    let receipts = load_receipts(&artifacts).unwrap();
    assert_eq!(receipts.len(), 1);
    assert!(receipts[0].receipt.is_ok());
}

#[test]
fn test_buildfix_directory_skipped() {
    let temp = create_temp_dir();
    let artifacts = artifacts_path(&temp);

    // Create buildfix's own output (should be skipped)
    create_receipt(&artifacts, "buildfix", valid_receipt());
    create_receipt(&artifacts, "builddiag", valid_receipt());

    let receipts = load_receipts(&artifacts).unwrap();

    // Should only load builddiag, not buildfix
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].sensor_id, "builddiag");
}

#[test]
fn test_cockpit_directory_skipped() {
    let temp = create_temp_dir();
    let artifacts = artifacts_path(&temp);

    create_receipt(&artifacts, "cockpit", valid_receipt());
    create_receipt(&artifacts, "builddiag", valid_receipt());

    let receipts = load_receipts(&artifacts).unwrap();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].sensor_id, "builddiag");
}

#[test]
fn test_nested_directories_not_matched() {
    let temp = create_temp_dir();
    let artifacts = artifacts_path(&temp);

    // Create a nested directory structure
    let nested = artifacts.join("deep").join("nested");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("report.json"), valid_receipt()).unwrap();

    // Also create a valid top-level receipt
    create_receipt(&artifacts, "top", valid_receipt());

    let receipts = load_receipts(&artifacts).unwrap();

    // Should only find the top-level one (glob pattern is */report.json)
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].sensor_id, "top");
}

#[test]
fn test_large_json_file() {
    let temp = create_temp_dir();
    let artifacts = artifacts_path(&temp);

    // Create a receipt with many findings
    let mut findings = Vec::new();
    for i in 0..1000 {
        findings.push(format!(
            r#"{{
                "severity": "warn",
                "check_id": "test.check",
                "code": "test_code",
                "message": "Finding number {}",
                "location": {{ "path": "src/lib.rs", "line": {} }}
            }}"#,
            i, i
        ));
    }

    let large_receipt = format!(
        r#"{{
            "schema": "test.report.v1",
            "tool": {{ "name": "test-sensor", "version": "1.0.0" }},
            "verdict": {{ "status": "warn", "counts": {{ "findings": 1000, "errors": 0, "warnings": 1000 }} }},
            "findings": [{}]
        }}"#,
        findings.join(",")
    );

    create_receipt(&artifacts, "large", &large_receipt);

    let receipts = load_receipts(&artifacts).unwrap();
    assert_eq!(receipts.len(), 1);
    assert!(receipts[0].receipt.is_ok());

    let envelope = receipts[0].receipt.as_ref().unwrap();
    assert_eq!(envelope.findings.len(), 1000);
}

#[test]
fn test_empty_json_object() {
    let temp = create_temp_dir();
    let artifacts = artifacts_path(&temp);

    create_receipt(&artifacts, "empty", "{}");

    let receipts = load_receipts(&artifacts).unwrap();
    assert_eq!(receipts.len(), 1);

    // Should fail - missing required fields
    assert!(matches!(
        receipts[0].receipt,
        Err(ReceiptLoadError::Json { .. })
    ));
}

#[test]
fn test_empty_file() {
    let temp = create_temp_dir();
    let artifacts = artifacts_path(&temp);

    create_receipt(&artifacts, "empty", "");

    let receipts = load_receipts(&artifacts).unwrap();
    assert_eq!(receipts.len(), 1);

    // Empty file is invalid JSON
    assert!(matches!(
        receipts[0].receipt,
        Err(ReceiptLoadError::Json { .. })
    ));
}

#[test]
fn test_null_json() {
    let temp = create_temp_dir();
    let artifacts = artifacts_path(&temp);

    create_receipt(&artifacts, "null", "null");

    let receipts = load_receipts(&artifacts).unwrap();
    assert_eq!(receipts.len(), 1);

    // null is valid JSON but not a valid receipt
    assert!(matches!(
        receipts[0].receipt,
        Err(ReceiptLoadError::Json { .. })
    ));
}

#[test]
fn test_findings_with_optional_fields() {
    let temp = create_temp_dir();
    let artifacts = artifacts_path(&temp);

    // Finding with minimal required fields
    let minimal_findings = r#"{
        "schema": "test.report.v1",
        "tool": { "name": "test", "version": "1.0.0" },
        "verdict": { "status": "warn", "counts": { "findings": 1, "errors": 0, "warnings": 1 } },
        "findings": [{
            "severity": "warn",
            "code": "test_code",
            "message": "Test message"
        }]
    }"#;

    create_receipt(&artifacts, "minimal", minimal_findings);

    let receipts = load_receipts(&artifacts).unwrap();
    assert!(receipts[0].receipt.is_ok());

    let envelope = receipts[0].receipt.as_ref().unwrap();
    assert_eq!(envelope.findings.len(), 1);
    assert!(envelope.findings[0].check_id.is_none());
    assert!(envelope.findings[0].location.is_none());
}
