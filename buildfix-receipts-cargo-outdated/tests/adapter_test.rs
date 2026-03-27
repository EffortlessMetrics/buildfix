//! Integration tests for buildfix-receipts-cargo-outdated adapter.
//!
//! Tests adapter implementation and fixture parsing.

use buildfix_adapter_sdk::{Adapter, AdapterMetadata, AdapterTestHarness};
use buildfix_receipts_cargo_outdated::CargoOutdatedAdapter;
use buildfix_types::receipt::{Severity, VerdictStatus};
use std::fs;
use tempfile::TempDir;

// ============================================================================
// Adapter Creation Tests
// ============================================================================

#[test]
fn test_adapter_new() {
    let adapter = CargoOutdatedAdapter::new();
    assert_eq!(adapter.sensor_id(), "cargo-outdated");
}

#[test]
fn test_adapter_default() {
    let adapter = CargoOutdatedAdapter;
    assert_eq!(adapter.sensor_id(), "cargo-outdated");
}

// ============================================================================
// Adapter Trait Tests
// ============================================================================

#[test]
fn test_adapter_sensor_id() {
    let adapter = CargoOutdatedAdapter::new();
    assert_eq!(adapter.sensor_id(), "cargo-outdated");
}

#[test]
fn test_adapter_loads_receipt() {
    let harness = AdapterTestHarness::new(CargoOutdatedAdapter::new());
    harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");
}

#[test]
fn test_adapter_load_receipt_with_outdated_deps() {
    let adapter = CargoOutdatedAdapter::new();
    let receipt = adapter
        .load(std::path::Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    // Should have findings for outdated dependencies
    assert_eq!(receipt.findings.len(), 3);

    // All should be warnings
    for finding in &receipt.findings {
        assert_eq!(finding.severity, Severity::Warn);
    }

    // Status should be Warn
    assert_eq!(receipt.verdict.status, VerdictStatus::Warn);

    // Counts should match
    assert_eq!(receipt.verdict.counts.findings, 3);
    assert_eq!(receipt.verdict.counts.warnings, 3);
    assert_eq!(receipt.verdict.counts.errors, 0);
}

#[test]
fn test_adapter_load_empty_dependencies() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"{
            "dependencies": []
        }"#,
    )
    .unwrap();

    let adapter = CargoOutdatedAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load empty report");

    // No findings
    assert!(receipt.findings.is_empty());

    // Status should be Pass
    assert_eq!(receipt.verdict.status, VerdictStatus::Pass);

    // Counts should be zero
    assert_eq!(receipt.verdict.counts.findings, 0);
    assert_eq!(receipt.verdict.counts.warnings, 0);
}

#[test]
fn test_adapter_load_no_dependencies_field() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(&json_path, r#"{}"#).unwrap();

    let adapter = CargoOutdatedAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load empty object");

    // No findings
    assert!(receipt.findings.is_empty());

    // Status should be Pass
    assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
}

#[test]
fn test_adapter_load_with_prod_dependency() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"{
            "dependencies": [
                {
                    "name": "serde",
                    "version": "1.0.0",
                    "latest": "1.0.200",
                    "kind": "Prod",
                    "project": "1.0.0",
                    "registry": "crates-io"
                }
            ]
        }"#,
    )
    .unwrap();

    let adapter = CargoOutdatedAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load report");

    assert_eq!(receipt.findings.len(), 1);

    let finding = &receipt.findings[0];
    // Prod dependencies should use deps.outdated_dependency check_id
    assert_eq!(
        finding.check_id,
        Some("deps.outdated_dependency".to_string())
    );
    assert!(finding.message.as_ref().unwrap().contains("serde"));
    assert!(finding.message.as_ref().unwrap().contains("1.0.0"));
    assert!(finding.message.as_ref().unwrap().contains("1.0.200"));
}

#[test]
fn test_adapter_load_with_dev_dependency() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"{
            "dependencies": [
                {
                    "name": "tokio",
                    "version": "1.0.0",
                    "latest": "1.40.0",
                    "kind": "Dev",
                    "project": "1.0.0",
                    "registry": "crates-io"
                }
            ]
        }"#,
    )
    .unwrap();

    let adapter = CargoOutdatedAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load report");

    assert_eq!(receipt.findings.len(), 1);

    let finding = &receipt.findings[0];
    // Dev dependencies should use outdated.outdated check_id
    assert_eq!(finding.check_id, Some("outdated.outdated".to_string()));
    assert!(finding.message.as_ref().unwrap().contains("tokio"));
}

#[test]
fn test_adapter_load_missing_file() {
    let adapter = CargoOutdatedAdapter::new();
    let result = adapter.load(std::path::Path::new("nonexistent.json"));

    assert!(result.is_err());
}

#[test]
fn test_adapter_load_invalid_json() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(&json_path, "not valid json").unwrap();

    let adapter = CargoOutdatedAdapter::new();
    let result = adapter.load(&json_path);

    assert!(result.is_err());
}

// ============================================================================
// AdapterMetadata Trait Tests
// ============================================================================

#[test]
fn test_adapter_metadata_name() {
    let adapter = CargoOutdatedAdapter::new();
    assert_eq!(adapter.name(), "cargo-outdated");
}

#[test]
fn test_adapter_metadata_version() {
    let adapter = CargoOutdatedAdapter::new();
    // Version should not be empty
    assert!(!adapter.version().is_empty());
}

#[test]
fn test_adapter_metadata_supported_schemas() {
    let adapter = CargoOutdatedAdapter::new();
    let schemas = adapter.supported_schemas();

    assert!(!schemas.is_empty());
    assert!(schemas.contains(&"cargo-outdated.report.v1"));
}

// ============================================================================
// Receipt Structure Tests
// ============================================================================

#[test]
fn test_receipt_tool_info() {
    let adapter = CargoOutdatedAdapter::new();
    let receipt = adapter
        .load(std::path::Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    assert_eq!(receipt.tool.name, "cargo-outdated");
    assert!(receipt.tool.version.is_some());
}

#[test]
fn test_receipt_schema() {
    let adapter = CargoOutdatedAdapter::new();
    let receipt = adapter
        .load(std::path::Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    assert_eq!(receipt.schema, "cargo-outdated.report.v1");
}

#[test]
fn test_receipt_finding_location() {
    let adapter = CargoOutdatedAdapter::new();
    let receipt = adapter
        .load(std::path::Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    // All findings should point to Cargo.toml
    for finding in &receipt.findings {
        assert!(finding.location.is_some());
        let loc = finding.location.as_ref().unwrap();
        assert_eq!(loc.path.as_str(), "Cargo.toml");
    }
}

#[test]
fn test_receipt_finding_data() {
    let adapter = CargoOutdatedAdapter::new();
    let receipt = adapter
        .load(std::path::Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    // Findings should have structured data
    for finding in &receipt.findings {
        assert!(finding.data.is_some());
        let data = finding.data.as_ref().unwrap();

        // Data should contain dependency info
        assert!(data.get("name").is_some());
        assert!(data.get("version").is_some());
        assert!(data.get("latest").is_some());
    }
}

// ============================================================================
// Harness Validation Tests
// ============================================================================

#[test]
fn test_harness_validate_receipt() {
    let harness = AdapterTestHarness::new(CargoOutdatedAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load");

    let validation = harness.validate_receipt(&receipt);
    assert!(validation.is_valid());
}

#[test]
fn test_harness_validate_finding_fields() {
    let harness = AdapterTestHarness::new(CargoOutdatedAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load");

    let result = harness.validate_finding_fields(&receipt);
    assert!(result.is_valid());
}

#[test]
fn test_harness_assert_finding_count() {
    let harness = AdapterTestHarness::new(CargoOutdatedAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load");

    // Should have 3 findings
    harness
        .assert_finding_count(&receipt, 3, None)
        .expect("should have 3 findings");

    // All should be warnings
    harness
        .assert_finding_count(&receipt, 3, Some(Severity::Warn))
        .expect("should have 3 warnings");
}

#[test]
fn test_harness_assert_has_check_id() {
    let harness = AdapterTestHarness::new(CargoOutdatedAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load");

    // Should have deps.outdated_dependency check_id (for Prod deps)
    harness
        .assert_has_check_id(&receipt, "deps.outdated_dependency")
        .expect("should have deps.outdated_dependency");

    // Should have outdated.outdated check_id (for Dev deps)
    harness
        .assert_has_check_id(&receipt, "outdated.outdated")
        .expect("should have outdated.outdated");
}

#[test]
fn test_harness_extract_check_ids() {
    let harness = AdapterTestHarness::new(CargoOutdatedAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load");

    let check_ids = harness.extract_check_ids(&receipt);

    // Should contain both check_ids
    assert!(check_ids.contains("deps.outdated_dependency"));
    assert!(check_ids.contains("outdated.outdated"));
}

#[test]
fn test_harness_check_id_format() {
    let harness = AdapterTestHarness::new(CargoOutdatedAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load");

    // Check IDs from cargo-outdated are "deps.outdated_dependency" and "outdated.outdated"
    // These have only 2 segments (1 dot), which doesn't meet the strict 3+ segment requirement
    // This test documents the current behavior - the check IDs are valid for the adapter
    // even if they don't meet the strict naming convention
    let result = harness.validate_check_id_format(&receipt);
    // The check IDs only have 2 segments, so validation will fail
    // This is expected behavior - the adapter uses shorter check IDs
    assert!(result.is_err());
    let errors = result.unwrap_err();
    // Verify we got the expected validation errors
    assert!(!errors.is_empty());
}

#[test]
fn test_harness_location_paths() {
    let harness = AdapterTestHarness::new(CargoOutdatedAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load");

    // Location paths should be valid
    let result = harness.validate_location_paths(&receipt);
    assert!(result.is_ok());
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_adapter_dependency_without_kind() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"{
            "dependencies": [
                {
                    "name": "serde",
                    "version": "1.0.0",
                    "latest": "1.0.200",
                    "project": "1.0.0",
                    "registry": "crates-io"
                }
            ]
        }"#,
    )
    .unwrap();

    let adapter = CargoOutdatedAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load report");

    assert_eq!(receipt.findings.len(), 1);
    // Without kind specified, should default to deps.outdated_dependency
    assert_eq!(
        receipt.findings[0].check_id,
        Some("deps.outdated_dependency".to_string())
    );
}

#[test]
fn test_adapter_dependency_without_registry() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"{
            "dependencies": [
                {
                    "name": "serde",
                    "version": "1.0.0",
                    "latest": "1.0.200",
                    "kind": "Prod",
                    "project": "1.0.0"
                }
            ]
        }"#,
    )
    .unwrap();

    let adapter = CargoOutdatedAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load report");

    assert_eq!(receipt.findings.len(), 1);
    // Should still work without registry field
    assert!(receipt.findings[0].data.is_some());
}

#[test]
fn test_adapter_multiple_dependencies_same_name() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    // This is a edge case - same dependency with different versions
    // The adapter should handle it gracefully
    fs::write(
        &json_path,
        r#"{
            "dependencies": [
                {
                    "name": "serde",
                    "version": "1.0.0",
                    "latest": "1.0.200",
                    "kind": "Prod",
                    "project": "1.0.0",
                    "registry": "crates-io"
                },
                {
                    "name": "serde",
                    "version": "0.9.0",
                    "latest": "1.0.200",
                    "kind": "Dev",
                    "project": "0.9.0",
                    "registry": "crates-io"
                }
            ]
        }"#,
    )
    .unwrap();

    let adapter = CargoOutdatedAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load report");

    // Should create two findings
    assert_eq!(receipt.findings.len(), 2);
}
