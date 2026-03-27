//! Integration tests for buildfix-receipts-depguard adapter.
//!
//! Tests adapter implementation and fixture parsing.

use buildfix_adapter_sdk::{Adapter, AdapterMetadata, AdapterTestHarness};
use buildfix_receipts_depguard::DepguardAdapter;
use buildfix_types::receipt::{Severity, VerdictStatus};
use std::fs;
use tempfile::TempDir;

// ============================================================================
// Adapter Creation Tests
// ============================================================================

#[test]
fn test_adapter_new() {
    let adapter = DepguardAdapter::new();
    assert_eq!(adapter.sensor_id(), "depguard");
}

#[test]
fn test_adapter_default() {
    let adapter = DepguardAdapter::default();
    assert_eq!(adapter.sensor_id(), "depguard");
}

// ============================================================================
// Adapter Trait Tests
// ============================================================================

#[test]
fn test_adapter_sensor_id() {
    let adapter = DepguardAdapter::new();
    assert_eq!(adapter.sensor_id(), "depguard");
}

#[test]
fn test_adapter_loads_receipt() {
    let harness = AdapterTestHarness::new(DepguardAdapter::new());
    harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load correctly");
}

#[test]
fn test_adapter_load_receipt_with_findings() {
    let adapter = DepguardAdapter::new();
    let receipt = adapter
        .load(std::path::Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    // Should have findings for depguard violations
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
fn test_adapter_load_empty_files_format() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"{
            "files": []
        }"#,
    )
    .unwrap();

    let adapter = DepguardAdapter::new();
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
fn test_adapter_load_empty_array_format() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    // Empty array is not a valid depguard format - it needs at least one item with manifest_path
    // to be recognized as array format. Test that it returns an error.
    fs::write(&json_path, r#"[]"#).unwrap();

    let adapter = DepguardAdapter::new();
    let result = adapter.load(&json_path);

    // Empty array without manifest_path is not recognized as valid format
    assert!(result.is_err());
}

#[test]
fn test_adapter_load_array_format_with_violations() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"[
            {
                "manifest_path": "/path/to/Cargo.toml",
                "violations": [
                    {
                        "dependency": "foo",
                        "type": "path_requires_version"
                    }
                ]
            }
        ]"#,
    )
    .unwrap();

    let adapter = DepguardAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load array report");

    assert_eq!(receipt.findings.len(), 1);

    let finding = &receipt.findings[0];
    assert_eq!(
        finding.check_id,
        Some("deps.path_requires_version".to_string())
    );
    assert!(finding.message.as_ref().unwrap().contains("foo"));
}

#[test]
fn test_adapter_load_files_format_with_messages() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"{
            "files": [
                {
                    "path": "/path/to/Cargo.toml",
                    "messages": [
                        {
                            "message": "path dependency bar should have a version",
                            "code": "E001",
                            "type": "path_requires_version",
                            "line": 10,
                            "column": 1
                        }
                    ]
                }
            ]
        }"#,
    )
    .unwrap();

    let adapter = DepguardAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load files report");

    assert_eq!(receipt.findings.len(), 1);

    let finding = &receipt.findings[0];
    assert_eq!(
        finding.check_id,
        Some("deps.path_requires_version".to_string())
    );
    assert_eq!(
        finding.location.as_ref().unwrap().path.as_str(),
        "/path/to/Cargo.toml"
    );
    assert_eq!(finding.location.as_ref().unwrap().line, Some(10));
    assert_eq!(finding.location.as_ref().unwrap().column, Some(1));
}

#[test]
fn test_adapter_load_missing_file() {
    let adapter = DepguardAdapter::new();
    let result = adapter.load(std::path::Path::new("nonexistent.json"));

    assert!(result.is_err());
}

#[test]
fn test_adapter_load_invalid_json() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(&json_path, "not valid json").unwrap();

    let adapter = DepguardAdapter::new();
    let result = adapter.load(&json_path);

    assert!(result.is_err());
}

#[test]
fn test_adapter_load_unknown_format() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(&json_path, r#"{"unknown": "format"}"#).unwrap();

    let adapter = DepguardAdapter::new();
    let result = adapter.load(&json_path);

    assert!(result.is_err());
}

// ============================================================================
// AdapterMetadata Trait Tests
// ============================================================================

#[test]
fn test_adapter_metadata_name() {
    let adapter = DepguardAdapter::new();
    assert_eq!(adapter.name(), "depguard");
}

#[test]
fn test_adapter_metadata_version() {
    let adapter = DepguardAdapter::new();
    // Version should not be empty
    assert!(!adapter.version().is_empty());
}

#[test]
fn test_adapter_metadata_supported_schemas() {
    let adapter = DepguardAdapter::new();
    let schemas = adapter.supported_schemas();

    assert!(!schemas.is_empty());
    assert!(schemas.contains(&"depguard.report.v1"));
}

// ============================================================================
// Receipt Structure Tests
// ============================================================================

#[test]
fn test_receipt_tool_info() {
    let adapter = DepguardAdapter::new();
    let receipt = adapter
        .load(std::path::Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    assert_eq!(receipt.tool.name, "depguard");
    assert!(receipt.tool.version.is_some());
}

#[test]
fn test_receipt_schema() {
    let adapter = DepguardAdapter::new();
    let receipt = adapter
        .load(std::path::Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    assert_eq!(receipt.schema, "depguard.report.v1");
}

#[test]
fn test_receipt_finding_location() {
    let adapter = DepguardAdapter::new();
    let receipt = adapter
        .load(std::path::Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    // All findings should have a location
    for finding in &receipt.findings {
        assert!(finding.location.is_some());
        let loc = finding.location.as_ref().unwrap();
        assert!(!loc.path.as_str().is_empty());
    }
}

#[test]
fn test_receipt_finding_data() {
    let adapter = DepguardAdapter::new();
    let receipt = adapter
        .load(std::path::Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    // Findings should have structured data
    for finding in &receipt.findings {
        assert!(finding.data.is_some());
    }
}

// ============================================================================
// Check ID Tests
// ============================================================================

#[test]
fn test_check_id_path_requires_version() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"[
            {
                "manifest_path": "/path/to/Cargo.toml",
                "violations": [
                    {
                        "dependency": "foo",
                        "type": "path_requires_version"
                    }
                ]
            }
        ]"#,
    )
    .unwrap();

    let adapter = DepguardAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load");

    assert_eq!(receipt.findings.len(), 1);
    assert_eq!(
        receipt.findings[0].check_id,
        Some("deps.path_requires_version".to_string())
    );
}

#[test]
fn test_check_id_workspace_inheritance() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"[
            {
                "manifest_path": "/path/to/Cargo.toml",
                "violations": [
                    {
                        "dependency": "bar",
                        "type": "workspace_inheritance"
                    }
                ]
            }
        ]"#,
    )
    .unwrap();

    let adapter = DepguardAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load");

    assert_eq!(receipt.findings.len(), 1);
    assert_eq!(
        receipt.findings[0].check_id,
        Some("deps.workspace_inheritance".to_string())
    );
}

#[test]
fn test_check_id_duplicate_dependency_versions() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"[
            {
                "manifest_path": "/path/to/Cargo.toml",
                "violations": [
                    {
                        "dependency": "baz",
                        "type": "duplicate_dependency_versions"
                    }
                ]
            }
        ]"#,
    )
    .unwrap();

    let adapter = DepguardAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load");

    assert_eq!(receipt.findings.len(), 1);
    assert_eq!(
        receipt.findings[0].check_id,
        Some("deps.duplicate_dependency_versions".to_string())
    );
}

#[test]
fn test_check_id_duplicate_versions_short() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"[
            {
                "manifest_path": "/path/to/Cargo.toml",
                "violations": [
                    {
                        "dependency": "baz",
                        "type": "duplicate_versions"
                    }
                ]
            }
        ]"#,
    )
    .unwrap();

    let adapter = DepguardAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load");

    assert_eq!(receipt.findings.len(), 1);
    assert_eq!(
        receipt.findings[0].check_id,
        Some("deps.duplicate_dependency_versions".to_string())
    );
}

#[test]
fn test_check_id_unknown_type() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"[
            {
                "manifest_path": "/path/to/Cargo.toml",
                "violations": [
                    {
                        "dependency": "foo",
                        "type": "unknown_check"
                    }
                ]
            }
        ]"#,
    )
    .unwrap();

    let adapter = DepguardAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load");

    assert_eq!(receipt.findings.len(), 1);
    assert_eq!(
        receipt.findings[0].check_id,
        Some("deps.unknown_check".to_string())
    );
}

// ============================================================================
// Harness Validation Tests
// ============================================================================

#[test]
fn test_harness_validate_receipt() {
    let harness = AdapterTestHarness::new(DepguardAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load");

    let validation = harness.validate_receipt(&receipt);
    assert!(validation.is_valid());
}

#[test]
fn test_harness_validate_finding_fields() {
    let harness = AdapterTestHarness::new(DepguardAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load");

    let result = harness.validate_finding_fields(&receipt);
    assert!(result.is_valid());
}

#[test]
fn test_harness_assert_finding_count() {
    let harness = AdapterTestHarness::new(DepguardAdapter::new());
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
    let harness = AdapterTestHarness::new(DepguardAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load");

    // Should have path_requires_version check_id
    harness
        .assert_has_check_id(&receipt, "deps.path_requires_version")
        .expect("should have deps.path_requires_version");

    // Should have workspace_inheritance check_id
    harness
        .assert_has_check_id(&receipt, "deps.workspace_inheritance")
        .expect("should have deps.workspace_inheritance");

    // Should have duplicate_dependency_versions check_id
    harness
        .assert_has_check_id(&receipt, "deps.duplicate_dependency_versions")
        .expect("should have deps.duplicate_dependency_versions");
}

#[test]
fn test_harness_extract_check_ids() {
    let harness = AdapterTestHarness::new(DepguardAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("should load");

    let check_ids = harness.extract_check_ids(&receipt);

    // Should contain all check_ids from fixture
    assert!(check_ids.contains("deps.path_requires_version"));
    assert!(check_ids.contains("deps.workspace_inheritance"));
    assert!(check_ids.contains("deps.duplicate_dependency_versions"));
}

#[test]
fn test_harness_location_paths() {
    let harness = AdapterTestHarness::new(DepguardAdapter::new());
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
fn test_adapter_files_format_no_messages() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"{
            "files": [
                {
                    "path": "/path/to/Cargo.toml"
                }
            ]
        }"#,
    )
    .unwrap();

    let adapter = DepguardAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load");

    // No findings when no messages
    assert!(receipt.findings.is_empty());
    assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
}

#[test]
fn test_adapter_array_format_no_violations() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"[
            {
                "manifest_path": "/path/to/Cargo.toml"
            }
        ]"#,
    )
    .unwrap();

    let adapter = DepguardAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load");

    // No findings when no violations
    assert!(receipt.findings.is_empty());
    assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
}

#[test]
fn test_adapter_multiple_violations_same_manifest() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"[
            {
                "manifest_path": "/path/to/Cargo.toml",
                "violations": [
                    {"dependency": "foo", "type": "path_requires_version"},
                    {"dependency": "bar", "type": "workspace_inheritance"},
                    {"dependency": "baz", "type": "duplicate_dependency_versions"}
                ]
            }
        ]"#,
    )
    .unwrap();

    let adapter = DepguardAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load");

    assert_eq!(receipt.findings.len(), 3);
    assert_eq!(receipt.verdict.counts.findings, 3);
    assert_eq!(receipt.verdict.counts.warnings, 3);
}

#[test]
fn test_adapter_violation_without_dependency() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"[
            {
                "manifest_path": "/path/to/Cargo.toml",
                "violations": [
                    {"type": "path_requires_version"}
                ]
            }
        ]"#,
    )
    .unwrap();

    let adapter = DepguardAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load");

    // Should still create finding even without dependency name
    assert_eq!(receipt.findings.len(), 1);
}

#[test]
fn test_adapter_message_without_code() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"{
            "files": [
                {
                    "path": "/path/to/Cargo.toml",
                    "messages": [
                        {
                            "message": "some message",
                            "type": "path_requires_version"
                        }
                    ]
                }
            ]
        }"#,
    )
    .unwrap();

    let adapter = DepguardAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load");

    assert_eq!(receipt.findings.len(), 1);
    // Code should be None when not provided
    assert!(receipt.findings[0].code.is_none());
}

#[test]
fn test_adapter_message_without_line_column() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"{
            "files": [
                {
                    "path": "/path/to/Cargo.toml",
                    "messages": [
                        {
                            "message": "some message",
                            "type": "path_requires_version"
                        }
                    ]
                }
            ]
        }"#,
    )
    .unwrap();

    let adapter = DepguardAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load");

    assert_eq!(receipt.findings.len(), 1);
    // Line and column should be None when not provided
    assert!(
        receipt.findings[0]
            .location
            .as_ref()
            .unwrap()
            .line
            .is_none()
    );
    assert!(
        receipt.findings[0]
            .location
            .as_ref()
            .unwrap()
            .column
            .is_none()
    );
}

#[test]
fn test_adapter_multiple_manifests() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("report.json");

    fs::write(
        &json_path,
        r#"[
            {
                "manifest_path": "/path/to/workspace/Cargo.toml",
                "violations": [
                    {"dependency": "foo", "type": "path_requires_version"}
                ]
            },
            {
                "manifest_path": "/path/to/workspace/crates/bar/Cargo.toml",
                "violations": [
                    {"dependency": "baz", "type": "workspace_inheritance"}
                ]
            }
        ]"#,
    )
    .unwrap();

    let adapter = DepguardAdapter::new();
    let receipt = adapter.load(&json_path).expect("should load");

    assert_eq!(receipt.findings.len(), 2);

    // Check locations are correct
    let paths: Vec<&str> = receipt
        .findings
        .iter()
        .map(|f| f.location.as_ref().unwrap().path.as_str())
        .collect();
    assert!(paths.contains(&"/path/to/workspace/Cargo.toml"));
    assert!(paths.contains(&"/path/to/workspace/crates/bar/Cargo.toml"));
}

#[test]
fn test_severity_always_warn() {
    let adapter = DepguardAdapter::new();
    let receipt = adapter
        .load(std::path::Path::new("tests/fixtures/report.json"))
        .expect("should load fixture");

    // All findings should have Warn severity
    for finding in &receipt.findings {
        assert_eq!(finding.severity, Severity::Warn);
    }
}
