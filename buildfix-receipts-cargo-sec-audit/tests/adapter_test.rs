use buildfix_adapter_sdk::Adapter;
use buildfix_receipts_cargo_sec_audit::CargoSecAuditAdapter;
use buildfix_types::receipt::{Severity, VerdictStatus};
use pretty_assertions::assert_eq;
use std::path::Path;

#[test]
fn test_adapter_loads_fixture() {
    let adapter = CargoSecAuditAdapter::new();
    let fixture_path = Path::new("tests/fixtures/report.json");

    let receipt = adapter.load(fixture_path).unwrap();

    assert_eq!(receipt.findings.len(), 3);
    assert_eq!(receipt.verdict.counts.findings, 3);
    assert_eq!(receipt.verdict.counts.errors, 1);
    assert_eq!(receipt.verdict.counts.warnings, 1);
    assert_eq!(receipt.verdict.status, VerdictStatus::Fail);
}

#[test]
fn test_adapter_finding_severities() {
    let adapter = CargoSecAuditAdapter::new();
    let fixture_path = Path::new("tests/fixtures/report.json");

    let receipt = adapter.load(fixture_path).unwrap();

    let high_severity = receipt
        .findings
        .iter()
        .find(|f| f.check_id.as_ref().unwrap().contains("RUSTSEC-0001-0001"))
        .unwrap();
    assert_eq!(high_severity.severity, Severity::Error);

    let medium_severity = receipt
        .findings
        .iter()
        .find(|f| f.check_id.as_ref().unwrap().contains("RUSTSEC-0002-0002"))
        .unwrap();
    assert_eq!(medium_severity.severity, Severity::Warn);

    let low_severity = receipt
        .findings
        .iter()
        .find(|f| f.check_id.as_ref().unwrap().contains("RUSTSEC-0003-0003"))
        .unwrap();
    assert_eq!(low_severity.severity, Severity::Info);
}

#[test]
fn test_adapter_check_id_format() {
    let adapter = CargoSecAuditAdapter::new();
    let fixture_path = Path::new("tests/fixtures/report.json");

    let receipt = adapter.load(fixture_path).unwrap();

    for finding in &receipt.findings {
        let check_id = finding.check_id.as_ref().unwrap();
        assert!(check_id.starts_with("security.RUSTSEC-"));
    }
}

#[test]
fn test_adapter_location() {
    let adapter = CargoSecAuditAdapter::new();
    let fixture_path = Path::new("tests/fixtures/report.json");

    let receipt = adapter.load(fixture_path).unwrap();

    for finding in &receipt.findings {
        let location = finding.location.as_ref().unwrap();
        assert_eq!(location.path.as_str(), "Cargo.toml");
    }
}
