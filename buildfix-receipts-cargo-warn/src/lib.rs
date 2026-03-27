use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, AdapterMetadata, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::Deserialize;
use std::path::Path;

pub struct CargoWarnAdapter;

impl CargoWarnAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CargoWarnAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for CargoWarnAdapter {
    fn sensor_id(&self) -> &str {
        "cargo-warn"
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        let report: CargoWarnReport = serde_json::from_str(&content).map_err(AdapterError::Json)?;
        convert_report(report)
    }
}

impl AdapterMetadata for CargoWarnAdapter {
    fn name(&self) -> &str {
        "cargo-warn"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn supported_schemas(&self) -> &[&str] {
        &["cargo-warn.report.v1"]
    }
}

fn convert_report(report: CargoWarnReport) -> Result<ReceiptEnvelope, AdapterError> {
    let mut findings = Vec::new();
    let mut warn_count = 0u64;
    let mut error_count = 0u64;

    for warning in &report.warnings {
        let check_id = map_code_to_check_id(&warning.code);

        let location = Location {
            path: Utf8PathBuf::from(&warning.manifest_path),
            line: None,
            column: None,
        };

        let severity = match warning.severity.as_str() {
            "error" => {
                error_count += 1;
                Severity::Error
            }
            _ => {
                warn_count += 1;
                Severity::Warn
            }
        };

        findings.push(Finding {
            severity,
            check_id: Some(check_id),
            code: Some(warning.code.clone()),
            message: Some(warning.message.clone()),
            location: Some(location),
            fingerprint: None,
            data: None,
            ..Default::default()
        });
    }

    let status = if error_count > 0 {
        VerdictStatus::Fail
    } else if warn_count > 0 {
        VerdictStatus::Warn
    } else {
        VerdictStatus::Pass
    };

    let mut builder = ReceiptBuilder::new("cargo-warn")
        .with_schema("cargo-warn.report.v1")
        .with_tool_version("0.0.0")
        .with_status(status)
        .with_counts(findings.len() as u64, error_count, warn_count);

    for finding in findings {
        builder = builder.with_finding(finding);
    }

    let receipt = builder.build();

    Ok(receipt)
}

fn map_code_to_check_id(code: &str) -> String {
    match code {
        "unused-dependency" => "warn.unused_dependency".to_string(),
        "transitive-dependency" => "warn.transitive_dependency".to_string(),
        _ => format!("warn.{}", code.replace('-', "_")),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct CargoWarnReport {
    warnings: Vec<CargoWarnWarning>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CargoWarnWarning {
    manifest_path: String,
    message: String,
    code: String,
    severity: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = CargoWarnAdapter::new();
        assert_eq!(adapter.sensor_id(), "cargo-warn");
    }

    #[test]
    fn test_convert_report_with_warnings() {
        let json = r#"{
            "warnings": [
                {
                    "manifestPath": "/path/to/Cargo.toml",
                    "message": "dependency dev-duplicate is unused",
                    "code": "unused-dependency",
                    "severity": "warning"
                },
                {
                    "manifestPath": "/path/to/Cargo.toml",
                    "message": "dependency workspace is transitive",
                    "code": "transitive-dependency",
                    "severity": "warning"
                }
            ]
        }"#;

        let report: CargoWarnReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 2);

        let finding1 = &receipt.findings[0];
        assert_eq!(finding1.severity, Severity::Warn);
        assert_eq!(
            finding1.check_id,
            Some("warn.unused_dependency".to_string())
        );

        let finding2 = &receipt.findings[1];
        assert_eq!(finding2.severity, Severity::Warn);
        assert_eq!(
            finding2.check_id,
            Some("warn.transitive_dependency".to_string())
        );

        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
        assert_eq!(receipt.verdict.counts.findings, 2);
        assert_eq!(receipt.verdict.counts.warnings, 2);
    }

    #[test]
    fn test_convert_report_empty_passes() {
        let json = r#"{
            "warnings": []
        }"#;

        let report: CargoWarnReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_report_with_error() {
        let json = r#"{
            "warnings": [
                {
                    "manifestPath": "/path/to/Cargo.toml",
                    "message": "some error occurred",
                    "code": "some-error",
                    "severity": "error"
                }
            ]
        }"#;

        let report: CargoWarnReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert_eq!(receipt.findings[0].severity, Severity::Error);
        assert_eq!(receipt.verdict.status, VerdictStatus::Fail);
        assert_eq!(receipt.verdict.counts.errors, 1);
    }

    #[test]
    fn test_convert_report_mixed_severity() {
        let json = r#"{
            "warnings": [
                {
                    "manifestPath": "/path/to/Cargo.toml",
                    "message": "a warning",
                    "code": "some-warning",
                    "severity": "warning"
                },
                {
                    "manifestPath": "/path/to/Cargo.toml",
                    "message": "an error",
                    "code": "some-error",
                    "severity": "error"
                }
            ]
        }"#;

        let report: CargoWarnReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 2);
        assert_eq!(receipt.verdict.status, VerdictStatus::Fail);
        assert_eq!(receipt.verdict.counts.warnings, 1);
        assert_eq!(receipt.verdict.counts.errors, 1);
    }
}
