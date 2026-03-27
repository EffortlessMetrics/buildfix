//! Test harness for validating adapter implementations.
//!
//! This module provides `AdapterTestHarness` which helps validate that adapter
//! implementations correctly produce receipts that conform to buildfix expectations.
//!
//! # Metadata Validation
//!
//! Use [`AdapterTestHarness::validate_metadata`] to ensure adapters properly
//! implement the [`AdapterMetadata`](crate::AdapterMetadata) trait:
//!
//! ```ignore
//! use buildfix_adapter_sdk::{AdapterTestHarness, AdapterMetadata};
//!
//! #[test]
//! fn test_metadata() {
//!     let harness = AdapterTestHarness::new(MyAdapter::new());
//!     harness.validate_metadata(&harness.adapter()).expect("metadata should be valid");
//! }
//! ```

use crate::{Adapter, AdapterError, AdapterMetadata};
use buildfix_types::receipt::{ReceiptEnvelope, Severity};
use std::collections::HashSet;
use std::path::Path;

/// Error returned when adapter metadata validation fails.
///
/// This error is produced by [`AdapterTestHarness::validate_metadata`] when
/// an adapter's metadata does not meet the required constraints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataValidationError {
    /// The field that failed validation.
    pub field: &'static str,
    /// A human-readable description of the validation failure.
    pub message: &'static str,
}

impl std::fmt::Display for MetadataValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Metadata validation failed for '{}': {}",
            self.field, self.message
        )
    }
}

impl std::error::Error for MetadataValidationError {}

/// Validation error with context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// The field that failed validation
    pub field: String,
    /// The invalid value
    pub value: String,
    /// Human-readable error message
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Validation error on '{}': {} (value: {:?})",
            self.field, self.message, self.value
        )
    }
}

impl std::error::Error for ValidationError {}

#[derive(Debug, Default)]
pub struct ValidationResult {
    errors: Vec<ValidationError>,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_error(
        &mut self,
        field: impl Into<String>,
        value: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.errors.push(ValidationError {
            field: field.into(),
            value: value.into(),
            message: message.into(),
        });
    }

    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn errors(&self) -> &[ValidationError] {
        &self.errors
    }

    pub fn expect_valid(self) -> Result<(), Vec<ValidationError>> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors)
        }
    }
}

pub struct AdapterTestHarness<A: Adapter> {
    adapter: A,
}

impl<A: Adapter> AdapterTestHarness<A> {
    pub fn new(adapter: A) -> Self {
        Self { adapter }
    }

    pub fn adapter(&self) -> &A {
        &self.adapter
    }

    pub fn validate_receipt(&self, receipt: &ReceiptEnvelope) -> ValidationResult {
        let mut result = ValidationResult::new();

        if receipt.schema.is_empty() {
            result.add_error("schema", "", "schema must not be empty");
        }

        if receipt.tool.name.is_empty() {
            result.add_error("tool.name", "", "tool name must not be empty");
        }

        result
    }

    pub fn validate_receipt_fixture(
        &self,
        fixture_path: impl AsRef<Path>,
    ) -> Result<ReceiptEnvelope, AdapterError> {
        let path = fixture_path.as_ref();
        let receipt = self.adapter.load(path)?;

        let validation = self.validate_receipt(&receipt);
        if !validation.is_valid() {
            for err in validation.errors() {
                eprintln!("Validation warning: {}", err);
            }
        }

        Ok(receipt)
    }

    pub fn validate_finding_fields(&self, receipt: &ReceiptEnvelope) -> ValidationResult {
        let mut result = ValidationResult::new();

        for (i, finding) in receipt.findings.iter().enumerate() {
            if finding.message.is_none() && finding.data.is_none() {
                result.add_error(
                    format!("finding[{}]", i),
                    "",
                    "finding must have either message or data",
                );
            }

            if let Some(ref loc) = finding.location
                && loc.path.as_str().is_empty()
            {
                result.add_error(
                    format!("finding[{}].location.path", i),
                    "",
                    "location path must not be empty",
                );
            }

            if finding.severity == Severity::Error && finding.check_id.is_none() {
                result.add_error(
                    format!("finding[{}]", i),
                    "",
                    "error findings should have a check_id for actionable fixes",
                );
            }
        }

        result
    }

    pub fn golden_test(
        &self,
        fixture_path: impl AsRef<Path>,
        expected: &ReceiptEnvelope,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let actual = self.validate_receipt_fixture(fixture_path)?;

        let expected_json = serde_json::to_string_pretty(expected)?;
        let actual_json = serde_json::to_string_pretty(&actual)?;

        assert_eq!(
            expected_json, actual_json,
            "Golden test failed: receipt does not match expected"
        );

        Ok(())
    }

    pub fn assert_finding_count(
        &self,
        receipt: &ReceiptEnvelope,
        expected: usize,
        severity: Option<Severity>,
    ) -> Result<(), AdapterError> {
        let actual = match severity {
            Some(s) => receipt.findings.iter().filter(|f| f.severity == s).count(),
            None => receipt.findings.len(),
        };

        if actual != expected {
            return Err(AdapterError::InvalidFormat(format!(
                "Expected {} findings (severity: {:?}), found {}",
                expected, severity, actual
            )));
        }

        Ok(())
    }

    pub fn assert_has_check_id(
        &self,
        receipt: &ReceiptEnvelope,
        check_id: &str,
    ) -> Result<(), AdapterError> {
        let found = receipt
            .findings
            .iter()
            .any(|f| f.check_id.as_deref() == Some(check_id));

        if !found {
            return Err(AdapterError::InvalidFormat(format!(
                "Expected finding with check_id '{}', but none found",
                check_id
            )));
        }

        Ok(())
    }

    pub fn extract_check_ids(&self, receipt: &ReceiptEnvelope) -> HashSet<String> {
        receipt
            .findings
            .iter()
            .filter_map(|f| f.check_id.clone())
            .collect()
    }

    /// Validates adapter metadata is properly configured.
    ///
    /// This method checks that the adapter's [`AdapterMetadata`] implementation
    /// returns valid, non-empty values for all required fields:
    ///
    /// - `name` must not be empty
    /// - `version` must not be empty
    /// - `supported_schemas` must contain at least one schema
    ///
    /// # Arguments
    ///
    /// * `adapter` - A reference to any type implementing [`AdapterMetadata`]
    ///
    /// # Returns
    ///
    /// `Ok(())` if all metadata validations pass.
    ///
    /// # Errors
    ///
    /// Returns a [`MetadataValidationError`] with the field name and error message
    /// if any validation fails.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use buildfix_adapter_sdk::{AdapterTestHarness, AdapterMetadata};
    ///
    /// struct MyAdapter;
    /// impl AdapterMetadata for MyAdapter {
    ///     fn name(&self) -> &str { "my-adapter" }
    ///     fn version(&self) -> &str { env!("CARGO_PKG_VERSION") }
    ///     fn supported_schemas(&self) -> &[&str] { &["my-adapter.report.v1"] }
    /// }
    ///
    /// let harness = AdapterTestHarness::new(MyAdapter);
    /// harness.validate_metadata(&MyAdapter)
    ///     .expect("adapter metadata should be valid");
    /// ```
    pub fn validate_metadata<M: AdapterMetadata>(
        &self,
        adapter: &M,
    ) -> Result<(), MetadataValidationError> {
        if adapter.name().is_empty() {
            return Err(MetadataValidationError {
                field: "name",
                message: "adapter name must not be empty",
            });
        }

        if adapter.version().is_empty() {
            return Err(MetadataValidationError {
                field: "version",
                message: "adapter version must not be empty",
            });
        }

        if adapter.supported_schemas().is_empty() {
            return Err(MetadataValidationError {
                field: "supported_schemas",
                message: "adapter must support at least one schema",
            });
        }

        Ok(())
    }

    /// Validates that all check IDs follow the naming convention: `sensor.category.specific`
    ///
    /// # Arguments
    /// * `receipt` - The receipt to validate
    ///
    /// # Returns
    /// * `Ok(())` if all check IDs are valid
    /// * `Err(Vec<ValidationError>)` with details of invalid IDs
    ///
    /// # Check ID Format Rules
    /// - Must be lowercase
    /// - Must contain at least 2 dots (3+ segments)
    /// - Each segment must be snake_case alphanumeric (with hyphens allowed)
    /// - Examples: `cargo-deny.ban.multiple-versions`, `machete.unused_dependency`
    pub fn validate_check_id_format(
        &self,
        receipt: &ReceiptEnvelope,
    ) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        for (i, finding) in receipt.findings.iter().enumerate() {
            if let Some(ref check_id) = finding.check_id
                && !Self::is_valid_check_id(check_id)
            {
                errors.push(ValidationError {
                    field: format!("finding[{}].check_id", i),
                    value: check_id.clone(),
                    message: "check_id must follow naming convention: lowercase, at least 2 dots (3+ segments), each segment must be snake_case alphanumeric (e.g., 'cargo-deny.ban.multiple-versions')".to_string(),
                });
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Checks if a check ID follows the naming convention.
    ///
    /// Valid format: `sensor.category.specific` where:
    /// - All lowercase
    /// - At least 2 dots (3+ segments)
    /// - Each segment is alphanumeric with hyphens or underscores allowed
    fn is_valid_check_id(check_id: &str) -> bool {
        // Must be lowercase
        if check_id != check_id.to_lowercase() {
            return false;
        }

        // Must contain at least 2 dots (3+ segments)
        let segments: Vec<&str> = check_id.split('.').collect();
        if segments.len() < 3 {
            return false;
        }

        // Each segment must be non-empty and contain only valid characters
        for segment in segments {
            if segment.is_empty() {
                return false;
            }
            // Allow alphanumeric, hyphens, and underscores
            if !segment
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return false;
            }
        }

        true
    }

    /// Validates that all location paths in findings are well-formed.
    ///
    /// # Arguments
    /// * `receipt` - The receipt to validate
    ///
    /// # Returns
    /// * `Ok(())` if all paths are valid
    /// * `Err(Vec<ValidationError>)` with details of invalid paths
    ///
    /// # Path Validation Rules
    /// - Must not be empty
    /// - Must use forward slashes (not backslashes)
    /// - Must not start with `/` (relative paths only)
    /// - Must not contain `..` (no parent directory traversal)
    /// - Examples: `src/main.rs`, `Cargo.toml`, `crates/domain/src/lib.rs`
    pub fn validate_location_paths(
        &self,
        receipt: &ReceiptEnvelope,
    ) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        for (i, finding) in receipt.findings.iter().enumerate() {
            if let Some(ref location) = finding.location {
                let path = location.path.as_str();

                // Must not be empty
                if path.is_empty() {
                    errors.push(ValidationError {
                        field: format!("finding[{}].location.path", i),
                        value: path.to_string(),
                        message: "location path must not be empty".to_string(),
                    });
                    continue;
                }

                // Must not contain backslashes
                if path.contains('\\') {
                    errors.push(ValidationError {
                        field: format!("finding[{}].location.path", i),
                        value: path.to_string(),
                        message: "location path must use forward slashes, not backslashes"
                            .to_string(),
                    });
                }

                // Must not start with `/` (relative paths only)
                if path.starts_with('/') {
                    errors.push(ValidationError {
                        field: format!("finding[{}].location.path", i),
                        value: path.to_string(),
                        message:
                            "location path must be relative, not absolute (cannot start with '/')"
                                .to_string(),
                    });
                }

                // Must not contain `..` (no parent directory traversal)
                if path.contains("..") {
                    errors.push(ValidationError {
                        field: format!("finding[{}].location.path", i),
                        value: path.to_string(),
                        message: "location path must not contain '..' (parent directory traversal not allowed)".to_string(),
                    });
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Runs all validation checks on a receipt.
    ///
    /// # Arguments
    /// * `receipt` - The receipt to validate
    ///
    /// # Returns
    /// * `Ok(())` if all validations pass
    /// * `Err(Vec<ValidationError>)` with all validation errors
    pub fn validate_all(&self, receipt: &ReceiptEnvelope) -> Result<(), Vec<ValidationError>> {
        let mut all_errors = Vec::new();

        // Basic receipt validation
        let receipt_result = self.validate_receipt(receipt);
        if let Err(errors) = receipt_result.expect_valid() {
            all_errors.extend(errors);
        }

        // Finding fields validation
        let finding_result = self.validate_finding_fields(receipt);
        if let Err(errors) = finding_result.expect_valid() {
            all_errors.extend(errors);
        }

        // Check ID format validation
        if let Err(errors) = self.validate_check_id_format(receipt) {
            all_errors.extend(errors);
        }

        // Location paths validation
        if let Err(errors) = self.validate_location_paths(receipt) {
            all_errors.extend(errors);
        }

        if all_errors.is_empty() {
            Ok(())
        } else {
            Err(all_errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AdapterError;
    use buildfix_types::receipt::Finding;
    use camino::Utf8PathBuf;
    use std::path::Path;
    use tempfile::TempDir;

    struct DummyAdapter;

    impl Adapter for DummyAdapter {
        fn sensor_id(&self) -> &str {
            "dummy"
        }

        fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
            let content = std::fs::read_to_string(path)?;
            serde_json::from_str(&content).map_err(AdapterError::Json)
        }
    }

    #[test]
    fn test_validation_empty_schema() {
        let harness = AdapterTestHarness::new(DummyAdapter);
        let receipt = ReceiptEnvelope {
            schema: String::new(),
            tool: buildfix_types::receipt::ToolInfo {
                name: "test".to_string(),
                version: None,
                repo: None,
                commit: None,
            },
            run: Default::default(),
            verdict: Default::default(),
            findings: vec![],
            capabilities: None,
            data: None,
        };

        let result = harness.validate_receipt(&receipt);
        assert!(!result.is_valid());
    }

    #[test]
    fn test_validation_empty_tool_name() {
        let harness = AdapterTestHarness::new(DummyAdapter);
        let receipt = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: buildfix_types::receipt::ToolInfo {
                name: String::new(),
                version: None,
                repo: None,
                commit: None,
            },
            run: Default::default(),
            verdict: Default::default(),
            findings: vec![],
            capabilities: None,
            data: None,
        };

        let result = harness.validate_receipt(&receipt);
        assert!(!result.is_valid());
    }

    #[test]
    fn test_validation_valid_receipt() {
        let harness = AdapterTestHarness::new(DummyAdapter);
        let receipt = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: buildfix_types::receipt::ToolInfo {
                name: "test-tool".to_string(),
                version: Some("1.0.0".to_string()),
                repo: None,
                commit: None,
            },
            run: Default::default(),
            verdict: Default::default(),
            findings: vec![],
            capabilities: None,
            data: None,
        };

        let result = harness.validate_receipt(&receipt);
        assert!(result.is_valid());
    }

    #[test]
    fn test_extract_check_ids() {
        let harness = AdapterTestHarness::new(DummyAdapter);
        let receipt = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: buildfix_types::receipt::ToolInfo {
                name: "test".to_string(),
                version: None,
                repo: None,
                commit: None,
            },
            run: Default::default(),
            verdict: Default::default(),
            findings: vec![
                Finding {
                    severity: Severity::Error,
                    check_id: Some("DENY001".to_string()),
                    code: None,
                    message: Some("error".to_string()),
                    location: None,
                    fingerprint: None,
                    data: None,
                    ..Default::default()
                },
                Finding {
                    severity: Severity::Warn,
                    check_id: Some("WARN001".to_string()),
                    code: None,
                    message: Some("warning".to_string()),
                    location: None,
                    fingerprint: None,
                    data: None,
                    ..Default::default()
                },
                Finding {
                    severity: Severity::Info,
                    check_id: None,
                    code: None,
                    message: Some("info".to_string()),
                    location: None,
                    fingerprint: None,
                    data: None,
                    ..Default::default()
                },
            ],
            capabilities: None,
            data: None,
        };

        let ids = harness.extract_check_ids(&receipt);
        assert!(ids.contains("DENY001"));
        assert!(ids.contains("WARN001"));
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_golden_test_matching() {
        let temp_dir = TempDir::new().unwrap();
        let receipt_path = temp_dir.path().join("report.json");

        let expected = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: buildfix_types::receipt::ToolInfo {
                name: "test".to_string(),
                version: Some("1.0.0".to_string()),
                repo: None,
                commit: None,
            },
            run: Default::default(),
            verdict: Default::default(),
            findings: vec![],
            capabilities: None,
            data: None,
        };

        let content = serde_json::to_string_pretty(&expected).unwrap();
        std::fs::write(&receipt_path, content).unwrap();

        let harness = AdapterTestHarness::new(DummyAdapter);
        let result = harness.golden_test(&receipt_path, &expected);

        assert!(result.is_ok());
    }

    #[test]
    fn test_assert_finding_count() {
        let harness = AdapterTestHarness::new(DummyAdapter);
        let receipt = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: buildfix_types::receipt::ToolInfo {
                name: "test".to_string(),
                version: None,
                repo: None,
                commit: None,
            },
            run: Default::default(),
            verdict: Default::default(),
            findings: vec![
                Finding {
                    severity: Severity::Error,
                    check_id: Some("ERR001".to_string()),
                    code: None,
                    message: Some("error1".to_string()),
                    location: None,
                    fingerprint: None,
                    data: None,
                    ..Default::default()
                },
                Finding {
                    severity: Severity::Error,
                    check_id: Some("ERR002".to_string()),
                    code: None,
                    message: Some("error2".to_string()),
                    location: None,
                    fingerprint: None,
                    data: None,
                    ..Default::default()
                },
                Finding {
                    severity: Severity::Warn,
                    check_id: Some("WARN001".to_string()),
                    code: None,
                    message: Some("warning".to_string()),
                    location: None,
                    fingerprint: None,
                    data: None,
                    ..Default::default()
                },
            ],
            capabilities: None,
            data: None,
        };

        let result = harness.assert_finding_count(&receipt, 3, None);
        assert!(result.is_ok());

        let result = harness.assert_finding_count(&receipt, 2, Some(Severity::Error));
        assert!(result.is_ok());

        let result = harness.assert_finding_count(&receipt, 1, Some(Severity::Warn));
        assert!(result.is_ok());

        let result = harness.assert_finding_count(&receipt, 5, None);
        assert!(result.is_err());
    }

    // ==================== Check ID Format Validation Tests ====================

    #[test]
    fn test_validate_check_id_format_valid() {
        let harness = AdapterTestHarness::new(DummyAdapter);
        let receipt = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: buildfix_types::receipt::ToolInfo {
                name: "test".to_string(),
                version: None,
                repo: None,
                commit: None,
            },
            run: Default::default(),
            verdict: Default::default(),
            findings: vec![
                Finding {
                    severity: Severity::Error,
                    check_id: Some("cargo-deny.ban.multiple-versions".to_string()),
                    code: None,
                    message: Some("error".to_string()),
                    location: None,
                    fingerprint: None,
                    data: None,
                    ..Default::default()
                },
                Finding {
                    severity: Severity::Warn,
                    check_id: Some("machete.unused_dependency.main".to_string()),
                    code: None,
                    message: Some("warning".to_string()),
                    location: None,
                    fingerprint: None,
                    data: None,
                    ..Default::default()
                },
            ],
            capabilities: None,
            data: None,
        };

        assert!(harness.validate_check_id_format(&receipt).is_ok());
    }

    #[test]
    fn test_validate_check_id_format_no_dots() {
        let harness = AdapterTestHarness::new(DummyAdapter);
        let receipt = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: buildfix_types::receipt::ToolInfo {
                name: "test".to_string(),
                version: None,
                repo: None,
                commit: None,
            },
            run: Default::default(),
            verdict: Default::default(),
            findings: vec![Finding {
                severity: Severity::Error,
                check_id: Some("simplecheck".to_string()),
                code: None,
                message: Some("error".to_string()),
                location: None,
                fingerprint: None,
                data: None,
                ..Default::default()
            }],
            capabilities: None,
            data: None,
        };

        let result = harness.validate_check_id_format(&receipt);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].value, "simplecheck");
    }

    #[test]
    fn test_validate_check_id_format_one_dot() {
        let harness = AdapterTestHarness::new(DummyAdapter);
        let receipt = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: buildfix_types::receipt::ToolInfo {
                name: "test".to_string(),
                version: None,
                repo: None,
                commit: None,
            },
            run: Default::default(),
            verdict: Default::default(),
            findings: vec![Finding {
                severity: Severity::Error,
                check_id: Some("sensor.check".to_string()),
                code: None,
                message: Some("error".to_string()),
                location: None,
                fingerprint: None,
                data: None,
                ..Default::default()
            }],
            capabilities: None,
            data: None,
        };

        let result = harness.validate_check_id_format(&receipt);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].value, "sensor.check");
    }

    #[test]
    fn test_validate_check_id_format_uppercase() {
        let harness = AdapterTestHarness::new(DummyAdapter);
        let receipt = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: buildfix_types::receipt::ToolInfo {
                name: "test".to_string(),
                version: None,
                repo: None,
                commit: None,
            },
            run: Default::default(),
            verdict: Default::default(),
            findings: vec![Finding {
                severity: Severity::Error,
                check_id: Some("Cargo-Deny.Ban.Multiple".to_string()),
                code: None,
                message: Some("error".to_string()),
                location: None,
                fingerprint: None,
                data: None,
                ..Default::default()
            }],
            capabilities: None,
            data: None,
        };

        let result = harness.validate_check_id_format(&receipt);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].value, "Cargo-Deny.Ban.Multiple");
    }

    #[test]
    fn test_validate_check_id_format_none() {
        let harness = AdapterTestHarness::new(DummyAdapter);
        let receipt = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: buildfix_types::receipt::ToolInfo {
                name: "test".to_string(),
                version: None,
                repo: None,
                commit: None,
            },
            run: Default::default(),
            verdict: Default::default(),
            findings: vec![Finding {
                severity: Severity::Info,
                check_id: None,
                code: None,
                message: Some("info".to_string()),
                location: None,
                fingerprint: None,
                data: None,
                ..Default::default()
            }],
            capabilities: None,
            data: None,
        };

        // No check_id means no validation error for check_id format
        assert!(harness.validate_check_id_format(&receipt).is_ok());
    }

    // ==================== Location Path Validation Tests ====================

    #[test]
    fn test_validate_location_paths_valid() {
        let harness = AdapterTestHarness::new(DummyAdapter);
        let receipt = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: buildfix_types::receipt::ToolInfo {
                name: "test".to_string(),
                version: None,
                repo: None,
                commit: None,
            },
            run: Default::default(),
            verdict: Default::default(),
            findings: vec![
                Finding {
                    severity: Severity::Error,
                    check_id: Some("test.check.id".to_string()),
                    code: None,
                    message: Some("error".to_string()),
                    location: Some(buildfix_types::receipt::Location {
                        path: Utf8PathBuf::from("src/main.rs"),
                        line: Some(1),
                        column: None,
                    }),
                    fingerprint: None,
                    data: None,
                    ..Default::default()
                },
                Finding {
                    severity: Severity::Warn,
                    check_id: Some("test.check.two".to_string()),
                    code: None,
                    message: Some("warning".to_string()),
                    location: Some(buildfix_types::receipt::Location {
                        path: Utf8PathBuf::from("crates/domain/src/lib.rs"),
                        line: None,
                        column: None,
                    }),
                    fingerprint: None,
                    data: None,
                    ..Default::default()
                },
            ],
            capabilities: None,
            data: None,
        };

        assert!(harness.validate_location_paths(&receipt).is_ok());
    }

    #[test]
    fn test_validate_location_paths_empty() {
        let harness = AdapterTestHarness::new(DummyAdapter);
        let receipt = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: buildfix_types::receipt::ToolInfo {
                name: "test".to_string(),
                version: None,
                repo: None,
                commit: None,
            },
            run: Default::default(),
            verdict: Default::default(),
            findings: vec![Finding {
                severity: Severity::Error,
                check_id: Some("test.check.id".to_string()),
                code: None,
                message: Some("error".to_string()),
                location: Some(buildfix_types::receipt::Location {
                    path: Utf8PathBuf::new(),
                    line: None,
                    column: None,
                }),
                fingerprint: None,
                data: None,
                ..Default::default()
            }],
            capabilities: None,
            data: None,
        };

        let result = harness.validate_location_paths(&receipt);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("empty"));
    }

    #[test]
    fn test_validate_location_paths_backslash() {
        let harness = AdapterTestHarness::new(DummyAdapter);
        let receipt = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: buildfix_types::receipt::ToolInfo {
                name: "test".to_string(),
                version: None,
                repo: None,
                commit: None,
            },
            run: Default::default(),
            verdict: Default::default(),
            findings: vec![Finding {
                severity: Severity::Error,
                check_id: Some("test.check.id".to_string()),
                code: None,
                message: Some("error".to_string()),
                location: Some(buildfix_types::receipt::Location {
                    path: Utf8PathBuf::from("src\\main.rs"),
                    line: None,
                    column: None,
                }),
                fingerprint: None,
                data: None,
                ..Default::default()
            }],
            capabilities: None,
            data: None,
        };

        let result = harness.validate_location_paths(&receipt);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("forward slashes"));
    }

    #[test]
    fn test_validate_location_paths_absolute() {
        let harness = AdapterTestHarness::new(DummyAdapter);
        let receipt = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: buildfix_types::receipt::ToolInfo {
                name: "test".to_string(),
                version: None,
                repo: None,
                commit: None,
            },
            run: Default::default(),
            verdict: Default::default(),
            findings: vec![Finding {
                severity: Severity::Error,
                check_id: Some("test.check.id".to_string()),
                code: None,
                message: Some("error".to_string()),
                location: Some(buildfix_types::receipt::Location {
                    path: Utf8PathBuf::from("/src/main.rs"),
                    line: None,
                    column: None,
                }),
                fingerprint: None,
                data: None,
                ..Default::default()
            }],
            capabilities: None,
            data: None,
        };

        let result = harness.validate_location_paths(&receipt);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("relative"));
    }

    #[test]
    fn test_validate_location_paths_parent_traversal() {
        let harness = AdapterTestHarness::new(DummyAdapter);
        let receipt = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: buildfix_types::receipt::ToolInfo {
                name: "test".to_string(),
                version: None,
                repo: None,
                commit: None,
            },
            run: Default::default(),
            verdict: Default::default(),
            findings: vec![Finding {
                severity: Severity::Error,
                check_id: Some("test.check.id".to_string()),
                code: None,
                message: Some("error".to_string()),
                location: Some(buildfix_types::receipt::Location {
                    path: Utf8PathBuf::from("../src/main.rs"),
                    line: None,
                    column: None,
                }),
                fingerprint: None,
                data: None,
                ..Default::default()
            }],
            capabilities: None,
            data: None,
        };

        let result = harness.validate_location_paths(&receipt);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains(".."));
    }

    #[test]
    fn test_validate_location_paths_no_location() {
        let harness = AdapterTestHarness::new(DummyAdapter);
        let receipt = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: buildfix_types::receipt::ToolInfo {
                name: "test".to_string(),
                version: None,
                repo: None,
                commit: None,
            },
            run: Default::default(),
            verdict: Default::default(),
            findings: vec![Finding {
                severity: Severity::Info,
                check_id: Some("test.check.id".to_string()),
                code: None,
                message: Some("info".to_string()),
                location: None,
                fingerprint: None,
                data: None,
                ..Default::default()
            }],
            capabilities: None,
            data: None,
        };

        // No location means no path validation error
        assert!(harness.validate_location_paths(&receipt).is_ok());
    }

    // ==================== Validate All Tests ====================

    #[test]
    fn test_validate_all_valid() {
        let harness = AdapterTestHarness::new(DummyAdapter);
        let receipt = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: buildfix_types::receipt::ToolInfo {
                name: "test".to_string(),
                version: Some("1.0.0".to_string()),
                repo: None,
                commit: None,
            },
            run: Default::default(),
            verdict: Default::default(),
            findings: vec![Finding {
                severity: Severity::Error,
                check_id: Some("cargo-deny.ban.multiple-versions".to_string()),
                code: None,
                message: Some("error".to_string()),
                location: Some(buildfix_types::receipt::Location {
                    path: Utf8PathBuf::from("Cargo.toml"),
                    line: None,
                    column: None,
                }),
                fingerprint: None,
                data: None,
                ..Default::default()
            }],
            capabilities: None,
            data: None,
        };

        assert!(harness.validate_all(&receipt).is_ok());
    }

    #[test]
    fn test_validate_all_multiple_errors() {
        let harness = AdapterTestHarness::new(DummyAdapter);
        let receipt = ReceiptEnvelope {
            schema: String::new(), // Invalid: empty schema
            tool: buildfix_types::receipt::ToolInfo {
                name: String::new(), // Invalid: empty name
                version: None,
                repo: None,
                commit: None,
            },
            run: Default::default(),
            verdict: Default::default(),
            findings: vec![Finding {
                severity: Severity::Error,
                check_id: Some("INVALID".to_string()), // Invalid: not enough dots
                code: None,
                message: Some("error".to_string()),
                location: Some(buildfix_types::receipt::Location {
                    path: Utf8PathBuf::from("/absolute/path.rs"), // Invalid: absolute path
                    line: None,
                    column: None,
                }),
                fingerprint: None,
                data: None,
                ..Default::default()
            }],
            capabilities: None,
            data: None,
        };

        let result = harness.validate_all(&receipt);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        // Should have multiple errors from different validators
        assert!(errors.len() >= 3);
    }

    // ==================== ValidationError Tests ====================

    #[test]
    fn test_validation_error_display() {
        let error = ValidationError {
            field: "test.field".to_string(),
            value: "invalid_value".to_string(),
            message: "Field is invalid".to_string(),
        };

        let display = format!("{}", error);
        assert!(display.contains("test.field"));
        assert!(display.contains("invalid_value"));
        assert!(display.contains("Field is invalid"));
    }
}
