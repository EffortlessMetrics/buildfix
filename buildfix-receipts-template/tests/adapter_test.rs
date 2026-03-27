//! Integration tests for the example-linter adapter template.
//!
//! These tests demonstrate the recommended testing patterns for buildfix adapters:
//!
//! 1. Basic adapter functionality (sensor_id, metadata)
//! 2. Loading receipts from fixture files
//! 3. Metadata validation
//! 4. Check ID format validation
//! 5. Location path validation
//! 6. Golden test pattern

use buildfix_adapter_sdk::{Adapter, AdapterMetadata, AdapterTestHarness};
use buildfix_receipts_template::ExampleLinterAdapter;
use buildfix_types::receipt::Severity;

/// Tests that the adapter can load a receipt from a fixture file.
///
/// This is the most basic test every adapter should have. It validates
/// that the adapter can successfully parse its expected input format.
#[test]
fn test_adapter_loads_receipt_from_fixture() {
    let harness = AdapterTestHarness::new(ExampleLinterAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");

    // Basic validation
    assert!(!receipt.schema.is_empty());
    assert!(!receipt.tool.name.is_empty());
}

/// Tests that the adapter correctly reports its sensor ID.
#[test]
fn test_adapter_sensor_id() {
    let adapter = ExampleLinterAdapter::new();
    assert_eq!(adapter.sensor_id(), "example-linter");
}

/// Tests that the adapter implements metadata correctly.
#[test]
fn test_adapter_metadata() {
    let adapter = ExampleLinterAdapter::new();

    // Name should match sensor_id
    assert_eq!(adapter.name(), "example-linter");

    // Version should be non-empty
    assert!(!adapter.version().is_empty());

    // Should support at least one schema
    assert!(!adapter.supported_schemas().is_empty());
}

/// Tests that metadata validation passes for a correctly implemented adapter.
#[test]
fn test_metadata_validation() {
    let harness = AdapterTestHarness::new(ExampleLinterAdapter::new());
    let metadata = harness.adapter();

    let result = harness.validate_metadata(metadata);
    assert!(result.is_ok(), "metadata validation should pass");
}

/// Tests that check IDs follow the expected format.
///
/// Check IDs should follow the pattern: `<tool>.<category>.<specific>`
#[test]
fn test_check_id_format() {
    let harness = AdapterTestHarness::new(ExampleLinterAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");

    for finding in &receipt.findings {
        if let Some(check_id) = &finding.check_id {
            // Check ID should start with tool name
            assert!(
                check_id.starts_with("example-linter."),
                "check_id '{}' should start with 'example-linter.'",
                check_id
            );

            // Check ID should have at least 3 parts (tool.category.specific)
            let parts: Vec<&str> = check_id.split('.').collect();
            assert!(
                parts.len() >= 3,
                "check_id '{}' should have at least 3 dot-separated parts",
                check_id
            );
        }
    }
}

/// Tests that location paths are normalized correctly.
///
/// Paths should be:
/// - Using forward slashes
/// - Without leading ./
/// - Relative to repository root
#[test]
fn test_location_path_normalization() {
    let harness = AdapterTestHarness::new(ExampleLinterAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");

    for finding in &receipt.findings {
        if let Some(location) = &finding.location {
            let path = location.path.as_str();

            // Should not have backslashes
            assert!(
                !path.contains('\\'),
                "path '{}' should not contain backslashes",
                path
            );

            // Should not start with ./
            assert!(
                !path.starts_with("./"),
                "path '{}' should not start with './'",
                path
            );
        }
    }
}

/// Tests that findings have the expected severity levels.
#[test]
fn test_finding_severities() {
    let harness = AdapterTestHarness::new(ExampleLinterAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");

    // The fixture has one error and one warning
    let has_error = receipt
        .findings
        .iter()
        .any(|f| f.severity == Severity::Error);
    let has_warning = receipt
        .findings
        .iter()
        .any(|f| f.severity == Severity::Warn);

    assert!(has_error, "fixture should contain at least one error");
    assert!(has_warning, "fixture should contain at least one warning");
}

/// Tests that the verdict status matches the findings.
#[test]
fn test_verdict_status_consistency() {
    let harness = AdapterTestHarness::new(ExampleLinterAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");

    // If there are errors, status should be Fail
    let has_errors = receipt
        .findings
        .iter()
        .any(|f| f.severity == Severity::Error);

    if has_errors {
        assert_eq!(
            receipt.verdict.status,
            buildfix_types::receipt::VerdictStatus::Fail
        );
    }
}

/// Tests that finding counts are accurate.
#[test]
fn test_finding_counts() {
    let harness = AdapterTestHarness::new(ExampleLinterAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");

    // Total findings should match
    assert_eq!(
        receipt.verdict.counts.findings as usize,
        receipt.findings.len()
    );

    // Count errors
    let error_count = receipt
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count() as u64;
    assert_eq!(receipt.verdict.counts.errors, error_count);

    // Count warnings
    let warn_count = receipt
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Warn)
        .count() as u64;
    assert_eq!(receipt.verdict.counts.warnings, warn_count);
}

/// Tests that findings have required fields populated.
#[test]
fn test_finding_required_fields() {
    let harness = AdapterTestHarness::new(ExampleLinterAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");

    for finding in &receipt.findings {
        // Every finding should have a message
        assert!(finding.message.is_some(), "finding should have a message");

        // Every finding should have a check_id
        assert!(finding.check_id.is_some(), "finding should have a check_id");

        // Every finding should have a location
        assert!(finding.location.is_some(), "finding should have a location");
    }
}

/// Golden test: validates that the output matches expected structure.
///
/// Golden tests capture the expected output and verify that future
/// changes don't accidentally alter the output format.
#[test]
fn test_golden_output_structure() {
    let harness = AdapterTestHarness::new(ExampleLinterAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");

    // Verify schema version
    assert_eq!(receipt.schema, "example-linter.report.v1");

    // Verify tool name
    assert_eq!(receipt.tool.name, "example-linter");

    // Verify findings structure
    for finding in &receipt.findings {
        // All fields should be serializable
        let json = serde_json::to_string(finding).expect("finding should be serializable");
        assert!(!json.is_empty());

        // Should be able to deserialize back
        let _: buildfix_types::receipt::Finding =
            serde_json::from_str(&json).expect("finding should deserialize");
    }
}
