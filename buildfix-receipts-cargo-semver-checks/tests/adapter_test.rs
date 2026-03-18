use buildfix_adapter_sdk::Adapter;
use buildfix_receipts_cargo_semver_checks::CargoSemverChecksAdapter;
use buildfix_types::receipt::VerdictStatus;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_adapter_loads_fixture() {
    let adapter = CargoSemverChecksAdapter::new();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("report.json");

    let receipt = adapter.load(&fixture).unwrap();

    assert_eq!(receipt.findings.len(), 3);
    assert_eq!(receipt.verdict.status, VerdictStatus::Fail);
    assert_eq!(receipt.verdict.counts.errors, 3);
    assert_eq!(receipt.verdict.counts.warnings, 0);
}

#[test]
fn test_adapter_finds_semver_codes() {
    let adapter = CargoSemverChecksAdapter::new();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("report.json");

    let receipt = adapter.load(&fixture).unwrap();

    let check_ids: Vec<_> = receipt
        .findings
        .iter()
        .filter_map(|f| f.check_id.clone())
        .collect();

    assert!(check_ids.iter().any(|c| c.contains("semver.RUSTSEMVER")));
}

#[test]
fn test_adapter_returns_correct_sensor_id() {
    let adapter = CargoSemverChecksAdapter::new();
    assert_eq!(adapter.sensor_id(), "cargo-semver-checks");
}

#[test]
fn test_adapter_handles_empty_semver_checks() {
    let adapter = CargoSemverChecksAdapter::new();
    let temp_dir = TempDir::new().unwrap();
    let empty_file = temp_dir.path().join("empty.json");
    std::fs::write(&empty_file, r#"{"semver_checks": []}"#).unwrap();

    let receipt = adapter.load(&empty_file).unwrap();

    assert_eq!(receipt.findings.len(), 0);
    assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
}

#[test]
fn test_adapter_handles_missing_errors_field() {
    let adapter = CargoSemverChecksAdapter::new();
    let temp_dir = TempDir::new().unwrap();
    let file = temp_dir.path().join("no_errors.json");
    std::fs::write(
        &file,
        r#"{"semver_checks": [{"package": "test", "version": "1.0.0"}]}"#,
    )
    .unwrap();

    let receipt = adapter.load(&file).unwrap();

    assert_eq!(receipt.findings.len(), 0);
    assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
}
