use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, AdapterMetadata, ReceiptBuilder};
use buildfix_types::receipt::{Finding, ReceiptEnvelope, Severity, VerdictStatus};
use serde::Deserialize;
use std::path::Path;

pub struct CargoLockAdapter;

impl CargoLockAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CargoLockAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for CargoLockAdapter {
    fn sensor_id(&self) -> &str {
        "cargo-lock"
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        let report: LockReport = serde_json::from_str(&content).map_err(AdapterError::Json)?;
        convert_report(report)
    }
}

impl AdapterMetadata for CargoLockAdapter {
    fn name(&self) -> &str {
        "cargo-lock"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn supported_schemas(&self) -> &[&str] {
        &["cargo-lock.report.v1"]
    }
}

fn convert_report(report: LockReport) -> Result<ReceiptEnvelope, AdapterError> {
    let mut findings = Vec::new();
    let mut warn_count = 0u64;

    if let Some(warnings) = &report.warnings {
        for warning in warnings {
            let message = warning.clone();
            let data = serde_json::json!({ "warning": message });

            findings.push(Finding {
                severity: Severity::Warn,
                check_id: Some("lock.warnings".to_string()),
                code: None,
                message: Some(message),
                location: None,
                fingerprint: None,
                data: Some(data),
                ..Default::default()
            });

            warn_count += 1;
        }
    }

    let status = if warn_count > 0 {
        VerdictStatus::Warn
    } else {
        VerdictStatus::Pass
    };

    let mut builder = ReceiptBuilder::new("cargo-lock")
        .with_schema("cargo-lock.report.v1")
        .with_tool_version("0.0.0")
        .with_status(status)
        .with_counts(findings.len() as u64, 0, warn_count);

    for finding in findings {
        builder = builder.with_finding(finding);
    }

    let receipt = builder.build();

    Ok(receipt)
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct LockReport {
    packages: Option<Vec<LockPackage>>,
    warnings: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
struct LockPackage {
    name: String,
    version: String,
    source: Option<String>,
    checksum: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = CargoLockAdapter::new();
        assert_eq!(adapter.sensor_id(), "cargo-lock");
    }

    #[test]
    fn test_convert_report_with_warnings() {
        let json = r#"{
            "packages": [
                {
                    "name": "serde",
                    "version": "1.0.200",
                    "source": "registry+https://github.com/rust-lang/crates.io-index",
                    "checksum": "abc123"
                }
            ],
            "warnings": ["some warning about dependencies"]
        }"#;

        let report: LockReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 1);

        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Warn);
        assert_eq!(finding.check_id, Some("lock.warnings".to_string()));
        assert!(finding.message.as_ref().unwrap().contains("some warning"));

        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
        assert_eq!(receipt.verdict.counts.findings, 1);
        assert_eq!(receipt.verdict.counts.warnings, 1);
    }

    #[test]
    fn test_convert_report_empty_passes() {
        let json = r#"{
            "packages": []
        }"#;

        let report: LockReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_report_no_packages_passes() {
        let json = r#"{}"#;

        let report: LockReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_adapter_loads_from_file() {
        let adapter = CargoLockAdapter::new();
        let receipt = adapter
            .load(Path::new("tests/fixtures/report.json"))
            .expect("should load fixture");

        assert_eq!(adapter.sensor_id(), "cargo-lock");
        assert!(!receipt.findings.is_empty());
    }

    #[test]
    fn test_adapter_returns_error_for_missing_file() {
        let adapter = CargoLockAdapter::new();
        let result = adapter.load(Path::new("nonexistent/path.json"));

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AdapterError::Io(_)));
    }

    #[test]
    fn test_adapter_returns_error_for_invalid_json() {
        let adapter = CargoLockAdapter::new();

        let temp_dir = tempfile::tempdir().unwrap();
        let temp_path = temp_dir.path().join("invalid.json");
        std::fs::write(&temp_path, "{ invalid json }").unwrap();

        let result = adapter.load(&temp_path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AdapterError::Json(_)));
    }

    #[test]
    fn test_finding_data_format() {
        let json = r#"{
            "packages": [],
            "warnings": ["test warning message"]
        }"#;

        let report: LockReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        let data = receipt.findings[0].data.as_ref().unwrap();
        assert_eq!(data.get("warning").unwrap(), "test warning message");
    }

    #[test]
    fn test_multiple_warnings() {
        let json = r#"{
            "packages": [],
            "warnings": ["warning 1", "warning 2", "warning 3"]
        }"#;

        let report: LockReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 3);
        assert_eq!(receipt.verdict.counts.warnings, 3);
    }
}
