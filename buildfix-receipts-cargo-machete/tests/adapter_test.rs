//! Integration tests for buildfix-receipts-cargo-machete adapter.
//!
//! Tests adapter implementation and fixture parsing.

use buildfix_adapter_sdk::{Adapter, AdapterMetadata, AdapterTestHarness};
use buildfix_receipts_cargo_machete::CargoMacheteAdapter;
use buildfix_types::receipt::{Severity, VerdictStatus};
use std::fs;
use tempfile::TempDir;

// ============================================================================
// Adapter Creation Tests
// ============================================================================

#[test]
fn test_adapter_new() {
    let adapter = CargoMacheteAdapter::new();
    assert_eq!(adapter.sensor_id(), "cargo-machete");
}

#[test]
fn test_adapter_default() {
    let adapter = CargoMacheteAdapter;
    assert_eq!(adapter.sensor_id(), "cargo-machete");
}

// ============================================================================
// Adapter Trait Tests
// ============================================================================

#[test]
fn test_adapter_sensor_id() {
    let adapter = CargoMacheteAdapter::new();
    assert_eq!(adapter.sensor_id(), "cargo-machete");
}

#[test]
fn test_adapter_loads_receipt() {
    let harness = AdapterTestHarness::new(CargoMacheteAdapter::new());
    harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");
}

#[test]
fn test_adapter_load_receipt_with_unused_deps() {
    let adapter = CargoMacheteAdapter::new();
    let receipt = adapter
        .load(std::path::Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    // Should have findings for unused dependencies
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
fn test_adapter_load_empty_crates() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"{
            "crates": []
        }"#,
    )
    .unwrap();

    let adapter = CargoMacheteAdapter::new();
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
fn test_adapter_load_no_crates_field() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(&json_path, r#"{}"#).unwrap();

    let adapter = CargoMacheteAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load empty object");

    // No findings
    assert!(receipt.findings.is_empty());

    // Status should be Pass
    assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
}

#[test]
fn test_adapter_load_with_direct_dependency() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"{
            "crates": [
                {
                    "name": "serde",
                    "manifest_path": "/path/to/Cargo.toml",
                    "kind": "direct"
                }
            ]
        }"#,
    )
    .unwrap();

    let adapter = CargoMacheteAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load report");

    assert_eq!(receipt.findings.len(), 1);

    let finding = &receipt.findings[0];
    assert_eq!(
        finding.check_id,
        Some("machete.unused_dependency".to_string())
    );
    assert!(finding.message.as_ref().unwrap().contains("serde"));
    assert!(finding.message.as_ref().unwrap().contains("direct"));
}

#[test]
fn test_adapter_load_with_transitive_dependency() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"{
            "crates": [
                {
                    "name": "tokio",
                    "manifest_path": "/path/to/Cargo.toml",
                    "kind": "transitive"
                }
            ]
        }"#,
    )
    .unwrap();

    let adapter = CargoMacheteAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load report");

    assert_eq!(receipt.findings.len(), 1);

    let finding = &receipt.findings[0];
    assert_eq!(
        finding.check_id,
        Some("machete.unused_dependency".to_string())
    );
    assert!(finding.message.as_ref().unwrap().contains("tokio"));
    assert!(finding.message.as_ref().unwrap().contains("transitive"));
}

#[test]
fn test_adapter_load_missing_file() {
    let adapter = CargoMacheteAdapter::new();
    let result = adapter.load(std::path::Path::new("nonexistent.json"));

    assert!(result.is_err());
}

#[test]
fn test_adapter_load_invalid_json() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(&json_path, "not valid json").unwrap();

    let adapter = CargoMacheteAdapter::new();
    let result = adapter.load(&json_path);

    assert!(result.is_err());
}

// ============================================================================
// AdapterMetadata Trait Tests
// ============================================================================

#[test]
fn test_adapter_metadata_name() {
    let adapter = CargoMacheteAdapter::new();
    assert_eq!(adapter.name(), "cargo-machete");
}

#[test]
fn test_adapter_metadata_version() {
    let adapter = CargoMacheteAdapter::new();
    // Version should not be empty
    assert!(!adapter.version().is_empty());
}

#[test]
fn test_adapter_metadata_supported_schemas() {
    let adapter = CargoMacheteAdapter::new();
    let schemas = adapter.supported_schemas();

    assert!(!schemas.is_empty());
    assert!(schemas.contains(&"cargo-machete.report.v1"));
}

// ============================================================================
// Receipt Structure Tests
// ============================================================================

#[test]
fn test_receipt_tool_info() {
    let adapter = CargoMacheteAdapter::new();
    let receipt = adapter
        .load(std::path::Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    assert_eq!(receipt.tool.name, "cargo-machete");
    assert!(receipt.tool.version.is_some());
}

#[test]
fn test_receipt_schema() {
    let adapter = CargoMacheteAdapter::new();
    let receipt = adapter
        .load(std::path::Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    assert_eq!(receipt.schema, "cargo-machete.report.v1");
}

#[test]
fn test_receipt_finding_location() {
    let adapter = CargoMacheteAdapter::new();
    let receipt = adapter
        .load(std::path::Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    // All findings should have a location pointing to a Cargo.toml
    for finding in &receipt.findings {
        assert!(finding.location.is_some());
        let loc = finding.location.as_ref().unwrap();
        assert!(loc.path.as_str().ends_with("Cargo.toml"));
    }

    // Verify specific paths from fixture
    let paths: Vec<&str> = receipt
        .findings
        .iter()
        .map(|f| f.location.as_ref().unwrap().path.as_str())
        .collect();
    assert!(paths.contains(&"Cargo.toml"));
    assert!(paths.contains(&"crates/foo/Cargo.toml"));
}

#[test]
fn test_receipt_finding_data() {
    let adapter = CargoMacheteAdapter::new();
    let receipt = adapter
        .load(std::path::Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    // Findings should have structured data
    for finding in &receipt.findings {
        assert!(finding.data.is_some());
        let data = finding.data.as_ref().unwrap();

        // Data should contain dependency info
        assert!(data.get("name").is_some());
        assert!(data.get("kind").is_some());
    }
}

// ============================================================================
// Check ID Tests
// ============================================================================

#[test]
fn test_check_id_unused_dependency() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"{
            "crates": [
                {
                    "name": "unused-crate",
                    "manifest_path": "/path/to/Cargo.toml",
                    "kind": "direct"
                }
            ]
        }"#,
    )
    .unwrap();

    let adapter = CargoMacheteAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load");

    assert_eq!(receipt.findings.len(), 1);
    assert_eq!(
        receipt.findings[0].check_id,
        Some("machete.unused_dependency".to_string())
    );
}

// ============================================================================
// Harness Validation Tests
// ============================================================================

#[test]
fn test_harness_validate_receipt() {
    let harness = AdapterTestHarness::new(CargoMacheteAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load");

    let validation = harness.validate_receipt(&receipt);
    assert!(validation.is_valid());
}

#[test]
fn test_harness_validate_finding_fields() {
    let harness = AdapterTestHarness::new(CargoMacheteAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load");

    let result = harness.validate_finding_fields(&receipt);
    assert!(result.is_valid());
}

#[test]
fn test_harness_assert_finding_count() {
    let harness = AdapterTestHarness::new(CargoMacheteAdapter::new());
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
    let harness = AdapterTestHarness::new(CargoMacheteAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load");

    // Should have machete.unused_dependency check_id
    harness
        .assert_has_check_id(&receipt, "machete.unused_dependency")
        .expect("should have machete.unused_dependency");
}

#[test]
fn test_harness_extract_check_ids() {
    let harness = AdapterTestHarness::new(CargoMacheteAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load");

    let check_ids = harness.extract_check_ids(&receipt);

    // Should contain machete.unused_dependency
    assert!(check_ids.contains("machete.unused_dependency"));
}

#[test]
fn test_harness_location_paths() {
    let harness = AdapterTestHarness::new(CargoMacheteAdapter::new());
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
fn test_adapter_multiple_crates_same_manifest() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"{
            "crates": [
                {
                    "name": "serde",
                    "manifest_path": "/path/to/Cargo.toml",
                    "kind": "direct"
                },
                {
                    "name": "tokio",
                    "manifest_path": "/path/to/Cargo.toml",
                    "kind": "direct"
                },
                {
                    "name": "rand",
                    "manifest_path": "/path/to/Cargo.toml",
                    "kind": "transitive"
                }
            ]
        }"#,
    )
    .unwrap();

    let adapter = CargoMacheteAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load");

    assert_eq!(receipt.findings.len(), 3);
    assert_eq!(receipt.verdict.counts.findings, 3);
    assert_eq!(receipt.verdict.counts.warnings, 3);
}

#[test]
fn test_adapter_multiple_manifests() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"{
            "crates": [
                {
                    "name": "serde",
                    "manifest_path": "/workspace/Cargo.toml",
                    "kind": "direct"
                },
                {
                    "name": "tokio",
                    "manifest_path": "/workspace/crates/foo/Cargo.toml",
                    "kind": "direct"
                }
            ]
        }"#,
    )
    .unwrap();

    let adapter = CargoMacheteAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load");

    assert_eq!(receipt.findings.len(), 2);

    // Check locations are correct
    let paths: Vec<&str> = receipt
        .findings
        .iter()
        .map(|f| f.location.as_ref().unwrap().path.as_str())
        .collect();
    assert!(paths.contains(&"/workspace/Cargo.toml"));
    assert!(paths.contains(&"/workspace/crates/foo/Cargo.toml"));
}

#[test]
fn test_adapter_finding_message_format() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"{
            "crates": [
                {
                    "name": "test-crate",
                    "manifest_path": "crates/test/Cargo.toml",
                    "kind": "direct"
                }
            ]
        }"#,
    )
    .unwrap();

    let adapter = CargoMacheteAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load");

    let msg = receipt.findings[0].message.as_ref().unwrap();
    assert!(msg.contains("test-crate"));
    assert!(msg.contains("direct"));
    assert!(msg.contains("unused dependency"));
}

#[test]
fn test_adapter_location_no_line_column() {
    let adapter = CargoMacheteAdapter::new();
    let receipt = adapter
        .load(std::path::Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    // cargo-machete doesn't provide line/column info
    for finding in &receipt.findings {
        assert!(finding.location.is_some());
        let loc = finding.location.as_ref().unwrap();
        assert!(loc.line.is_none());
        assert!(loc.column.is_none());
    }
}

#[test]
fn test_adapter_with_confidence_field() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"{
            "crates": [
                {
                    "name": "test-crate",
                    "manifest_path": "/path/to/Cargo.toml",
                    "kind": "direct",
                    "confidence": 0.95
                }
            ]
        }"#,
    )
    .unwrap();

    let adapter = CargoMacheteAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load");

    assert_eq!(receipt.findings.len(), 1);
    // Confidence should be preserved
    assert!(receipt.findings[0].confidence.is_some());
    assert_eq!(receipt.findings[0].confidence.unwrap(), 0.95);
}

#[test]
fn test_severity_always_warn() {
    let adapter = CargoMacheteAdapter::new();
    let receipt = adapter
        .load(std::path::Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    // All findings should have Warn severity
    for finding in &receipt.findings {
        assert_eq!(finding.severity, Severity::Warn);
    }
}

#[test]
fn test_adapter_all_direct_kind() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"{
            "crates": [
                {
                    "name": "crate1",
                    "manifest_path": "/path/to/Cargo.toml",
                    "kind": "direct"
                },
                {
                    "name": "crate2",
                    "manifest_path": "/path/to/Cargo.toml",
                    "kind": "direct"
                }
            ]
        }"#,
    )
    .unwrap();

    let adapter = CargoMacheteAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load");

    assert_eq!(receipt.findings.len(), 2);
    for finding in &receipt.findings {
        assert!(finding.message.as_ref().unwrap().contains("direct"));
    }
}

#[test]
fn test_adapter_all_transitive_kind() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"{
            "crates": [
                {
                    "name": "crate1",
                    "manifest_path": "/path/to/Cargo.toml",
                    "kind": "transitive"
                },
                {
                    "name": "crate2",
                    "manifest_path": "/path/to/Cargo.toml",
                    "kind": "transitive"
                }
            ]
        }"#,
    )
    .unwrap();

    let adapter = CargoMacheteAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load");

    assert_eq!(receipt.findings.len(), 2);
    for finding in &receipt.findings {
        assert!(finding.message.as_ref().unwrap().contains("transitive"));
    }
}

#[test]
fn test_adapter_data_fields() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"{
            "crates": [
                {
                    "name": "serde",
                    "manifest_path": "/path/to/Cargo.toml",
                    "kind": "direct"
                }
            ]
        }"#,
    )
    .unwrap();

    let adapter = CargoMacheteAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load");

    assert_eq!(receipt.findings.len(), 1);

    let data = receipt.findings[0].data.as_ref().unwrap();
    assert_eq!(data.get("name").unwrap().as_str().unwrap(), "serde");
    assert_eq!(data.get("kind").unwrap().as_str().unwrap(), "direct");
}
