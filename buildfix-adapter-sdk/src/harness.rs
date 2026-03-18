//! Test harness for validating adapter implementations.
//!
//! This module provides `AdapterTestHarness` which helps validate that adapter
//! implementations correctly produce receipts that conform to buildfix expectations.

use crate::{Adapter, AdapterError};
use buildfix_types::receipt::{ReceiptEnvelope, Severity};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Validation error on '{}': {}", self.field, self.message)
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

    pub fn add_error(&mut self, field: impl Into<String>, message: impl Into<String>) {
        self.errors.push(ValidationError {
            field: field.into(),
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
            result.add_error("schema", "schema must not be empty");
        }

        if receipt.tool.name.is_empty() {
            result.add_error("tool.name", "tool name must not be empty");
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
                    "finding must have either message or data",
                );
            }

            if let Some(ref loc) = finding.location
                && loc.path.as_str().is_empty()
            {
                result.add_error(
                    format!("finding[{}].location.path", i),
                    "location path must not be empty",
                );
            }

            if finding.severity == Severity::Error && finding.check_id.is_none() {
                result.add_error(
                    format!("finding[{}]", i),
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AdapterError;
    use buildfix_types::receipt::Finding;
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
                },
                Finding {
                    severity: Severity::Warn,
                    check_id: Some("WARN001".to_string()),
                    code: None,
                    message: Some("warning".to_string()),
                    location: None,
                    fingerprint: None,
                    data: None,
                },
                Finding {
                    severity: Severity::Info,
                    check_id: None,
                    code: None,
                    message: Some("info".to_string()),
                    location: None,
                    fingerprint: None,
                    data: None,
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
                },
                Finding {
                    severity: Severity::Error,
                    check_id: Some("ERR002".to_string()),
                    code: None,
                    message: Some("error2".to_string()),
                    location: None,
                    fingerprint: None,
                    data: None,
                },
                Finding {
                    severity: Severity::Warn,
                    check_id: Some("WARN001".to_string()),
                    code: None,
                    message: Some("warning".to_string()),
                    location: None,
                    fingerprint: None,
                    data: None,
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
}
