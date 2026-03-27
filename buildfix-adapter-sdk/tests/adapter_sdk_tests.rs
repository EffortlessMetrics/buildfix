//! Integration tests for buildfix-adapter-sdk.
//!
//! Tests the Adapter trait, AdapterMetadata trait, ReceiptBuilder, and harness validation functions.

use buildfix_adapter_sdk::{
    Adapter, AdapterError, AdapterMetadata, AdapterTestHarness, ReceiptBuilder,
};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use std::path::Path;
use tempfile::TempDir;

/// Helper function to create a simple finding (since simple_finding is not publicly exported)
fn simple_finding(
    message: impl Into<String>,
    path: impl Into<Utf8PathBuf>,
    line: u64,
    severity: Severity,
) -> Finding {
    Finding {
        severity,
        check_id: None,
        code: None,
        message: Some(message.into()),
        location: Some(Location {
            path: path.into(),
            line: Some(line),
            column: None,
        }),
        fingerprint: None,
        data: None,
        confidence: None,
        provenance: None,
        context: None,
    }
}

// ============================================================================
// Mock Adapter for Testing
// ============================================================================

struct MockAdapter {
    sensor_id: String,
}

impl MockAdapter {
    fn new() -> Self {
        Self {
            sensor_id: "mock-sensor".to_string(),
        }
    }

    fn with_id(id: &str) -> Self {
        Self {
            sensor_id: id.to_string(),
        }
    }
}

impl Default for MockAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for MockAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        let receipt: ReceiptEnvelope =
            serde_json::from_str(&content).map_err(AdapterError::Json)?;
        Ok(receipt)
    }
}

impl AdapterMetadata for MockAdapter {
    fn name(&self) -> &str {
        "mock-sensor"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn supported_schemas(&self) -> &[&str] {
        &["mock-sensor.report.v1", "mock-sensor.report.v2"]
    }
}

// ============================================================================
// Adapter Trait Tests
// ============================================================================

#[test]
fn test_adapter_sensor_id() {
    let adapter = MockAdapter::new();
    assert_eq!(adapter.sensor_id(), "mock-sensor");
}

#[test]
fn test_adapter_custom_sensor_id() {
    let adapter = MockAdapter::with_id("custom-sensor");
    assert_eq!(adapter.sensor_id(), "custom-sensor");
}

#[test]
fn test_adapter_load_valid_receipt() {
    let temp_dir = TempDir::new().unwrap();
    let receipt_path = temp_dir.path().join("report.json");

    let receipt = ReceiptBuilder::new("mock-sensor")
        .with_status(VerdictStatus::Pass)
        .build();

    let json = serde_json::to_string_pretty(&receipt).unwrap();
    std::fs::write(&receipt_path, json).unwrap();

    let adapter = MockAdapter::new();
    let loaded = adapter.load(&receipt_path).unwrap();

    assert_eq!(loaded.tool.name, "mock-sensor");
    assert_eq!(loaded.verdict.status, VerdictStatus::Pass);
}

#[test]
fn test_adapter_load_missing_file() {
    let adapter = MockAdapter::new();
    let result = adapter.load(Path::new("nonexistent.json"));

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, AdapterError::Io(_)));
}

#[test]
fn test_adapter_load_invalid_json() {
    let temp_dir = TempDir::new().unwrap();
    let receipt_path = temp_dir.path().join("report.json");

    std::fs::write(&receipt_path, "not valid json").unwrap();

    let adapter = MockAdapter::new();
    let result = adapter.load(&receipt_path);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, AdapterError::Json(_)));
}

// ============================================================================
// AdapterMetadata Trait Tests
// ============================================================================

#[test]
fn test_adapter_metadata_name() {
    let adapter = MockAdapter::new();
    assert_eq!(adapter.name(), "mock-sensor");
}

#[test]
fn test_adapter_metadata_version() {
    let adapter = MockAdapter::new();
    // Version should not be empty
    assert!(!adapter.version().is_empty());
}

#[test]
fn test_adapter_metadata_supported_schemas() {
    let adapter = MockAdapter::new();
    let schemas = adapter.supported_schemas();

    assert!(!schemas.is_empty());
    assert!(schemas.contains(&"mock-sensor.report.v1"));
    assert!(schemas.contains(&"mock-sensor.report.v2"));
}

// ============================================================================
// ReceiptBuilder Tests
// ============================================================================

#[test]
fn test_receipt_builder_basic() {
    let receipt = ReceiptBuilder::new("test-sensor")
        .with_status(VerdictStatus::Fail)
        .build();

    assert_eq!(receipt.tool.name, "test-sensor");
    assert_eq!(receipt.verdict.status, VerdictStatus::Fail);
    assert!(!receipt.schema.is_empty());
}

#[test]
fn test_receipt_builder_with_tool_version() {
    let receipt = ReceiptBuilder::new("test-sensor")
        .with_tool_version("1.2.3")
        .build();

    assert_eq!(receipt.tool.version, Some("1.2.3".to_string()));
}

#[test]
fn test_receipt_builder_with_tool_repo() {
    let receipt = ReceiptBuilder::new("test-sensor")
        .with_tool_repo("https://github.com/example/repo")
        .build();

    assert_eq!(
        receipt.tool.repo,
        Some("https://github.com/example/repo".to_string())
    );
}

#[test]
fn test_receipt_builder_with_tool_commit() {
    let receipt = ReceiptBuilder::new("test-sensor")
        .with_tool_commit("abc123")
        .build();

    assert_eq!(receipt.tool.commit, Some("abc123".to_string()));
}

#[test]
fn test_receipt_builder_with_schema() {
    let receipt = ReceiptBuilder::new("test-sensor")
        .with_schema("custom.schema.v1")
        .build();

    assert_eq!(receipt.schema, "custom.schema.v1");
}

#[test]
fn test_receipt_builder_with_counts() {
    let receipt = ReceiptBuilder::new("test-sensor")
        .with_counts(10, 3, 2)
        .build();

    assert_eq!(receipt.verdict.counts.findings, 10);
    assert_eq!(receipt.verdict.counts.errors, 3);
    assert_eq!(receipt.verdict.counts.warnings, 2);
}

#[test]
fn test_receipt_builder_with_reason() {
    let receipt = ReceiptBuilder::new("test-sensor")
        .with_reason("First reason")
        .with_reason("Second reason")
        .build();

    assert_eq!(receipt.verdict.reasons.len(), 2);
    assert!(
        receipt
            .verdict
            .reasons
            .contains(&"First reason".to_string())
    );
    assert!(
        receipt
            .verdict
            .reasons
            .contains(&"Second reason".to_string())
    );
}

#[test]
fn test_receipt_builder_with_check_id() {
    let receipt = ReceiptBuilder::new("test-sensor")
        .with_check_id("check.001")
        .with_check_id("check.002")
        .build();

    let caps = receipt.capabilities.expect("capabilities should be set");
    assert_eq!(caps.check_ids.len(), 2);
    assert!(caps.check_ids.contains(&"check.001".to_string()));
    assert!(caps.check_ids.contains(&"check.002".to_string()));
}

#[test]
fn test_receipt_builder_with_scope() {
    let receipt = ReceiptBuilder::new("test-sensor")
        .with_scope("workspace")
        .build();

    let caps = receipt.capabilities.expect("capabilities should be set");
    assert_eq!(caps.scopes, vec!["workspace"]);
}

#[test]
fn test_receipt_builder_with_partial() {
    let receipt = ReceiptBuilder::new("test-sensor")
        .with_partial(true)
        .with_partial_reason("Some files skipped")
        .build();

    let caps = receipt.capabilities.expect("capabilities should be set");
    assert!(caps.partial);
    assert_eq!(caps.reason, Some("Some files skipped".to_string()));
}

#[test]
fn test_receipt_builder_with_finding() {
    let finding = simple_finding("Test error", "src/main.rs", 10, Severity::Error);

    let receipt = ReceiptBuilder::new("test-sensor")
        .with_finding(finding)
        .build();

    assert_eq!(receipt.findings.len(), 1);
    assert_eq!(receipt.findings[0].severity, Severity::Error);
    assert_eq!(receipt.findings[0].message, Some("Test error".to_string()));
}

#[test]
fn test_receipt_builder_with_finding_at() {
    let receipt = ReceiptBuilder::new("test-sensor")
        .with_finding_at("src/lib.rs", 42, "Warning message", Severity::Warn)
        .build();

    assert_eq!(receipt.findings.len(), 1);
    let finding = &receipt.findings[0];
    assert_eq!(finding.severity, Severity::Warn);
    assert_eq!(finding.message, Some("Warning message".to_string()));
    assert!(finding.location.is_some());
    let loc = finding.location.as_ref().unwrap();
    assert_eq!(loc.path.as_str(), "src/lib.rs");
    assert_eq!(loc.line, Some(42));
}

#[test]
fn test_receipt_builder_finding_count_auto() {
    // When counts not explicitly set, findings count should match actual findings
    let receipt = ReceiptBuilder::new("test-sensor")
        .with_finding_at("a.rs", 1, "err1", Severity::Error)
        .with_finding_at("b.rs", 2, "err2", Severity::Warn)
        .with_finding_at("c.rs", 3, "err3", Severity::Info)
        .build();

    assert_eq!(receipt.verdict.counts.findings, 3);
}

#[test]
fn test_simple_finding() {
    let finding = simple_finding("Test message", "Cargo.toml", 5, Severity::Error);

    assert_eq!(finding.severity, Severity::Error);
    assert_eq!(finding.message, Some("Test message".to_string()));
    assert!(finding.location.is_some());
    let loc = finding.location.as_ref().unwrap();
    assert_eq!(loc.path.as_str(), "Cargo.toml");
    assert_eq!(loc.line, Some(5));
    assert_eq!(loc.column, None);
    assert!(finding.check_id.is_none());
    assert!(finding.code.is_none());
}

// ============================================================================
// AdapterTestHarness Tests
// ============================================================================

#[test]
fn test_harness_new() {
    let adapter = MockAdapter::new();
    let harness = AdapterTestHarness::new(adapter);

    assert_eq!(harness.adapter().sensor_id(), "mock-sensor");
}

#[test]
fn test_harness_validate_receipt_valid() {
    let adapter = MockAdapter::new();
    let harness = AdapterTestHarness::new(adapter);

    let receipt = ReceiptBuilder::new("mock-sensor")
        .with_schema("mock-sensor.report.v1")
        .build();

    let result = harness.validate_receipt(&receipt);
    assert!(result.is_valid());
}

#[test]
fn test_harness_validate_receipt_empty_schema() {
    let adapter = MockAdapter::new();
    let harness = AdapterTestHarness::new(adapter);

    let mut receipt = ReceiptBuilder::new("mock-sensor").build();
    receipt.schema = "".to_string();

    let result = harness.validate_receipt(&receipt);
    assert!(!result.is_valid());
    assert!(result.errors().iter().any(|e| e.field == "schema"));
}

#[test]
fn test_harness_validate_receipt_empty_tool_name() {
    let adapter = MockAdapter::new();
    let harness = AdapterTestHarness::new(adapter);

    let mut receipt = ReceiptBuilder::new("mock-sensor").build();
    receipt.tool.name = "".to_string();

    let result = harness.validate_receipt(&receipt);
    assert!(!result.is_valid());
    assert!(result.errors().iter().any(|e| e.field == "tool.name"));
}

#[test]
fn test_harness_validate_receipt_fixture() {
    let temp_dir = TempDir::new().unwrap();
    let receipt_path = temp_dir.path().join("report.json");

    let receipt = ReceiptBuilder::new("mock-sensor")
        .with_schema("mock-sensor.report.v1")
        .build();

    let json = serde_json::to_string_pretty(&receipt).unwrap();
    std::fs::write(&receipt_path, json).unwrap();

    let adapter = MockAdapter::new();
    let harness = AdapterTestHarness::new(adapter);

    let loaded = harness
        .validate_receipt_fixture(&receipt_path)
        .expect("receipt should load");

    assert_eq!(loaded.tool.name, "mock-sensor");
}

#[test]
fn test_harness_validate_metadata_valid() {
    let adapter = MockAdapter::new();
    let harness = AdapterTestHarness::new(adapter);

    let result = harness.validate_metadata(&MockAdapter::new());
    assert!(result.is_ok());
}

#[test]
fn test_harness_assert_finding_count() {
    let adapter = MockAdapter::new();
    let harness = AdapterTestHarness::new(adapter);

    let receipt = ReceiptBuilder::new("test-sensor")
        .with_finding_at("a.rs", 1, "err1", Severity::Error)
        .with_finding_at("b.rs", 2, "err2", Severity::Warn)
        .build();

    // Total findings
    harness
        .assert_finding_count(&receipt, 2, None)
        .expect("should have 2 findings");

    // By severity
    harness
        .assert_finding_count(&receipt, 1, Some(Severity::Error))
        .expect("should have 1 error");

    harness
        .assert_finding_count(&receipt, 1, Some(Severity::Warn))
        .expect("should have 1 warning");
}

#[test]
fn test_harness_assert_finding_count_mismatch() {
    let adapter = MockAdapter::new();
    let harness = AdapterTestHarness::new(adapter);

    let receipt = ReceiptBuilder::new("test-sensor").build();

    let result = harness.assert_finding_count(&receipt, 5, None);
    assert!(result.is_err());
}

#[test]
fn test_harness_assert_has_check_id() {
    let adapter = MockAdapter::new();
    let harness = AdapterTestHarness::new(adapter);

    let finding = simple_finding("test", "a.rs", 1, Severity::Error);
    let mut finding_with_check = finding;
    finding_with_check.check_id = Some("test.check.id".to_string());

    let receipt = ReceiptBuilder::new("test-sensor")
        .with_finding(finding_with_check)
        .build();

    harness
        .assert_has_check_id(&receipt, "test.check.id")
        .expect("should have check_id");
}

#[test]
fn test_harness_assert_has_check_id_missing() {
    let adapter = MockAdapter::new();
    let harness = AdapterTestHarness::new(adapter);

    let receipt = ReceiptBuilder::new("test-sensor")
        .with_finding_at("a.rs", 1, "err", Severity::Error)
        .build();

    let result = harness.assert_has_check_id(&receipt, "nonexistent.check.id");
    assert!(result.is_err());
}

#[test]
fn test_harness_extract_check_ids() {
    use std::collections::HashSet;

    let adapter = MockAdapter::new();
    let harness = AdapterTestHarness::new(adapter);

    let mut f1 = simple_finding("test1", "a.rs", 1, Severity::Error);
    f1.check_id = Some("check.id.one".to_string());

    let mut f2 = simple_finding("test2", "b.rs", 2, Severity::Error);
    f2.check_id = Some("check.id.two".to_string());

    let receipt = ReceiptBuilder::new("test-sensor")
        .with_finding(f1)
        .with_finding(f2)
        .build();

    let check_ids = harness.extract_check_ids(&receipt);

    let expected: HashSet<String> = ["check.id.one".to_string(), "check.id.two".to_string()]
        .into_iter()
        .collect();
    assert_eq!(check_ids, expected);
}

#[test]
fn test_harness_validate_finding_fields() {
    let adapter = MockAdapter::new();
    let harness = AdapterTestHarness::new(adapter);

    // Valid finding with message (using Warn since Error requires check_id)
    let receipt = ReceiptBuilder::new("test-sensor")
        .with_finding_at("a.rs", 1, "warning message", Severity::Warn)
        .build();

    let result = harness.validate_finding_fields(&receipt);
    assert!(result.is_valid());
}

#[test]
fn test_harness_validate_finding_fields_error_needs_check_id() {
    let adapter = MockAdapter::new();
    let harness = AdapterTestHarness::new(adapter);

    // Error without check_id should fail validation
    let receipt = ReceiptBuilder::new("test-sensor")
        .with_finding_at("a.rs", 1, "error message", Severity::Error)
        .build();

    let result = harness.validate_finding_fields(&receipt);
    // Error findings should have check_id for actionable fixes
    assert!(!result.is_valid());
}

#[test]
fn test_harness_validate_check_id_format() {
    let adapter = MockAdapter::new();
    let harness = AdapterTestHarness::new(adapter);

    let mut finding = simple_finding("test", "a.rs", 1, Severity::Error);
    finding.check_id = Some("sensor.category.specific".to_string());

    let receipt = ReceiptBuilder::new("test-sensor")
        .with_finding(finding)
        .build();

    let result = harness.validate_check_id_format(&receipt);
    assert!(result.is_ok());
}

#[test]
fn test_harness_validate_check_id_format_invalid() {
    let adapter = MockAdapter::new();
    let harness = AdapterTestHarness::new(adapter);

    let mut finding = simple_finding("test", "a.rs", 1, Severity::Error);
    finding.check_id = Some("invalid-check-id".to_string()); // Missing dots

    let receipt = ReceiptBuilder::new("test-sensor")
        .with_finding(finding)
        .build();

    let result = harness.validate_check_id_format(&receipt);
    assert!(result.is_err());
}

#[test]
fn test_harness_validate_location_paths() {
    let adapter = MockAdapter::new();
    let harness = AdapterTestHarness::new(adapter);

    let receipt = ReceiptBuilder::new("test-sensor")
        .with_finding_at("src/main.rs", 1, "error", Severity::Error)
        .build();

    let result = harness.validate_location_paths(&receipt);
    assert!(result.is_ok());
}

// ============================================================================
// ValidationResult Tests
// ============================================================================

#[test]
fn test_validation_result_new() {
    use buildfix_adapter_sdk::ValidationResult;

    let result = ValidationResult::new();
    assert!(result.is_valid());
    assert!(result.errors().is_empty());
}

#[test]
fn test_validation_result_add_error() {
    use buildfix_adapter_sdk::ValidationResult;

    let mut result = ValidationResult::new();
    result.add_error("field", "value", "error message");

    assert!(!result.is_valid());
    assert_eq!(result.errors().len(), 1);
    assert_eq!(result.errors()[0].field, "field");
    assert_eq!(result.errors()[0].value, "value");
    assert_eq!(result.errors()[0].message, "error message");
}

#[test]
fn test_validation_result_expect_valid() {
    use buildfix_adapter_sdk::ValidationResult;

    let result = ValidationResult::new();
    assert!(result.expect_valid().is_ok());

    let mut result = ValidationResult::new();
    result.add_error("field", "value", "error");
    assert!(result.expect_valid().is_err());
}

// ============================================================================
// MetadataValidationError Tests
// ============================================================================

#[test]
fn test_metadata_validation_error_display() {
    use buildfix_adapter_sdk::MetadataValidationError;

    let err = MetadataValidationError {
        field: "name",
        message: "must not be empty",
    };

    let display = format!("{}", err);
    assert!(display.contains("name"));
    assert!(display.contains("must not be empty"));
}

// ============================================================================
// AdapterError Tests
// ============================================================================

#[test]
fn test_adapter_error_io() {
    let err = AdapterError::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "file not found",
    ));
    let display = format!("{}", err);
    assert!(display.contains("IO error"));
}

#[test]
fn test_adapter_error_json() {
    let err = AdapterError::Json(serde_json::from_str::<serde_json::Value>("invalid").unwrap_err());
    let display = format!("{}", err);
    assert!(display.contains("JSON parse error"));
}

#[test]
fn test_adapter_error_invalid_format() {
    let err = AdapterError::InvalidFormat("bad format".to_string());
    let display = format!("{}", err);
    assert!(display.contains("Invalid sensor output"));
    assert!(display.contains("bad format"));
}

#[test]
fn test_adapter_error_missing_field() {
    let err = AdapterError::MissingField("required_field".to_string());
    let display = format!("{}", err);
    assert!(display.contains("Required field missing"));
    assert!(display.contains("required_field"));
}
