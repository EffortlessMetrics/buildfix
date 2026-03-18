use buildfix_adapter_sdk::Adapter;
use buildfix_receipts_sarif::SarifAdapter;
use buildfix_types::receipt::{Severity, VerdictStatus};
use std::path::Path;

#[test]
fn test_adapter_loads_sarif_fixture() {
    let adapter = SarifAdapter::new();
    let path = Path::new("tests/fixtures/sample.sarif");

    let receipt = adapter.load(path).expect("should load SARIF file");

    assert_eq!(receipt.tool.name, "sarif-codeanalyzer");
    assert_eq!(receipt.tool.version, Some("2.3.1".to_string()));
}

#[test]
fn test_adapter_extracts_findings() {
    let adapter = SarifAdapter::new();
    let path = Path::new("tests/fixtures/sample.sarif");

    let receipt = adapter.load(path).expect("should load SARIF file");

    assert_eq!(receipt.findings.len(), 3);
}

#[test]
fn test_adapter_maps_severity() {
    let adapter = SarifAdapter::new();
    let path = Path::new("tests/fixtures/sample.sarif");

    let receipt = adapter.load(path).expect("should load SARIF file");

    assert_eq!(receipt.findings[0].severity, Severity::Error);
    assert_eq!(receipt.findings[1].severity, Severity::Warn);
    assert_eq!(receipt.findings[2].severity, Severity::Info);
}

#[test]
fn test_adapter_extracts_location() {
    let adapter = SarifAdapter::new();
    let path = Path::new("tests/fixtures/sample.sarif");

    let receipt = adapter.load(path).expect("should load SARIF file");

    let first = &receipt.findings[0];
    let loc = first.location.as_ref().expect("should have location");
    assert_eq!(loc.path.as_str(), "src/database/query.rs");
    assert_eq!(loc.line, Some(127));
    assert_eq!(loc.column, Some(15));
}

#[test]
fn test_adapter_calculates_verdict() {
    let adapter = SarifAdapter::new();
    let path = Path::new("tests/fixtures/sample.sarif");

    let receipt = adapter.load(path).expect("should load SARIF file");

    assert_eq!(receipt.verdict.status, VerdictStatus::Fail);
    assert_eq!(receipt.verdict.counts.findings, 3);
    assert_eq!(receipt.verdict.counts.errors, 1);
    assert_eq!(receipt.verdict.counts.warnings, 1);
}

#[test]
fn test_adapter_with_tool_name() {
    let adapter = SarifAdapter::new().with_tool_name("semgrep");
    let path = Path::new("tests/fixtures/sample.sarif");

    let receipt = adapter.load(path).expect("should load SARIF file");

    assert_eq!(receipt.tool.name, "sarif-semgrep");
}
