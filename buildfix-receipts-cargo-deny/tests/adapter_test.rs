use buildfix_adapter_sdk::AdapterTestHarness;
use buildfix_receipts_cargo_deny::CargoDenyAdapter;
use pretty_assertions::assert_eq;

#[test]
fn test_adapter_loads_fixture() {
    let adapter = CargoDenyAdapter::new();
    let harness = AdapterTestHarness::new(adapter);

    harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");
}

#[test]
fn test_adapter_extracts_check_ids() {
    let adapter = CargoDenyAdapter::new();
    let harness = AdapterTestHarness::new(adapter);

    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load receipt");

    let check_ids = harness.extract_check_ids(&receipt);

    assert!(
        check_ids.contains("licenses.unlicensed"),
        "should have licenses.unlicensed"
    );
    assert!(
        check_ids.contains("bans.multi-usage"),
        "should have bans.multi-usage"
    );
    assert!(
        check_ids.contains("bans.circular"),
        "should have bans.circular"
    );
    assert!(
        check_ids.contains("bans.multiple-versions"),
        "should have bans.multiple-versions"
    );
}

#[test]
fn test_adapter_validates_finding_fields() {
    let adapter = CargoDenyAdapter::new();
    let harness = AdapterTestHarness::new(adapter);

    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load receipt");

    let result = harness.validate_finding_fields(&receipt);
    result
        .expect_valid()
        .expect("all findings should have required fields");
}

#[test]
fn test_adapter_finding_severity_mapping() {
    let adapter = CargoDenyAdapter::new();
    let harness = AdapterTestHarness::new(adapter);

    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load without errors");

    let deny_findings: Vec<_> = receipt
        .findings
        .iter()
        .filter(|f| f.severity == buildfix_types::receipt::Severity::Error)
        .collect();

    let warn_findings: Vec<_> = receipt
        .findings
        .iter()
        .filter(|f| f.severity == buildfix_types::receipt::Severity::Warn)
        .collect();

    assert_eq!(deny_findings.len(), 4, "should have 4 deny findings");
    assert_eq!(warn_findings.len(), 3, "should have 3 warn findings");
}
