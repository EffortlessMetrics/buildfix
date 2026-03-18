use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub struct CargoMsrvAdapter;

impl CargoMsrvAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CargoMsrvAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for CargoMsrvAdapter {
    fn sensor_id(&self) -> &str {
        "cargo-msrv"
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        let report: MsrvReport = serde_json::from_str(&content).map_err(AdapterError::Json)?;
        convert_report(report)
    }
}

fn convert_report(report: MsrvReport) -> Result<ReceiptEnvelope, AdapterError> {
    let mut findings = Vec::new();
    let mut warn_count = 0u64;

    if let Some(results) = &report.results {
        for result in results {
            let (check_id, message) = match result.status.as_str() {
                "incompatible" => (
                    "msrv.incompatible".to_string(),
                    format!(
                        "Crate '{}' v{} requires MSRV {} but the specified MSRV is {}",
                        result.name,
                        result.version,
                        result.msrv,
                        report.minimum_supported_rust_version.as_str()
                    ),
                ),
                "compatible" => continue,
                _ => (
                    "msrv.outdated".to_string(),
                    format!(
                        "Crate '{}' v{} has MSRV {} which is outdated",
                        result.name, result.version, result.msrv
                    ),
                ),
            };

            let location = Location {
                path: Utf8PathBuf::from("Cargo.toml"),
                line: None,
                column: None,
            };

            let data = MsrvResultData {
                name: result.name.clone(),
                version: result.version.clone(),
                msrv: result.msrv.clone(),
                rustc: result.rustc.clone(),
                status: result.status.clone(),
            };

            findings.push(Finding {
                severity: Severity::Warn,
                check_id: Some(check_id),
                code: None,
                message: Some(message),
                location: Some(location),
                fingerprint: None,
                data: Some(serde_json::to_value(data).unwrap_or_default()),
            });

            warn_count += 1;
        }
    }

    let status = if warn_count > 0 {
        VerdictStatus::Warn
    } else {
        VerdictStatus::Pass
    };

    let mut builder = ReceiptBuilder::new("cargo-msrv")
        .with_schema("cargo-msrv.report.v1")
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
struct MsrvReport {
    results: Option<Vec<MsrvResult>>,
    #[serde(default)]
    minimum_supported_rust_version: String,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
struct MsrvResult {
    name: String,
    version: String,
    msrv: String,
    rustc: String,
    status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MsrvResultData {
    name: String,
    version: String,
    msrv: String,
    rustc: String,
    status: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = CargoMsrvAdapter::new();
        assert_eq!(adapter.sensor_id(), "cargo-msrv");
    }

    #[test]
    fn test_convert_report_with_incompatible() {
        let json = r#"{
            "results": [
                {
                    "name": "my-crate",
                    "version": "1.0.0",
                    "msrv": "1.56.0",
                    "rustc": "1.56.0",
                    "status": "incompatible"
                }
            ],
            "minimum_supported_rust_version": "1.40.0"
        }"#;

        let report: MsrvReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 1);

        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Warn);
        assert_eq!(finding.check_id, Some("msrv.incompatible".to_string()));
        assert!(finding.message.as_ref().unwrap().contains("my-crate"));
        assert!(finding.message.as_ref().unwrap().contains("1.56.0"));
        assert!(finding.message.as_ref().unwrap().contains("1.40.0"));

        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
        assert_eq!(receipt.verdict.counts.findings, 1);
        assert_eq!(receipt.verdict.counts.warnings, 1);
    }

    #[test]
    fn test_convert_report_with_outdated() {
        let json = r#"{
            "results": [
                {
                    "name": "tokio",
                    "version": "1.0.0",
                    "msrv": "1.56.0",
                    "rustc": "1.56.0",
                    "status": "unknown"
                }
            ],
            "minimum_supported_rust_version": "1.56.0"
        }"#;

        let report: MsrvReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 1);

        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Warn);
        assert_eq!(finding.check_id, Some("msrv.outdated".to_string()));
        assert!(finding.message.as_ref().unwrap().contains("tokio"));

        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
    }

    #[test]
    fn test_convert_report_all_compatible_passes() {
        let json = r#"{
            "results": [
                {
                    "name": "serde",
                    "version": "1.0.0",
                    "msrv": "1.56.0",
                    "rustc": "1.56.0",
                    "status": "compatible"
                }
            ],
            "minimum_supported_rust_version": "1.56.0"
        }"#;

        let report: MsrvReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_report_empty_passes() {
        let json = r#"{
            "results": [],
            "minimum_supported_rust_version": "1.56.0"
        }"#;

        let report: MsrvReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_report_no_results_passes() {
        let json = r#"{}"#;

        let report: MsrvReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_finding_location_is_cargo_toml() {
        let json = r#"{
            "results": [
                {
                    "name": "test-crate",
                    "version": "1.0.0",
                    "msrv": "1.56.0",
                    "rustc": "1.56.0",
                    "status": "incompatible"
                }
            ],
            "minimum_supported_rust_version": "1.40.0"
        }"#;

        let report: MsrvReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        let location = receipt.findings[0].location.as_ref().unwrap();
        assert_eq!(location.path.as_str(), "Cargo.toml");
    }

    #[test]
    fn test_finding_data_contains_info() {
        let json = r#"{
            "results": [
                {
                    "name": "test-crate",
                    "version": "1.0.0",
                    "msrv": "1.56.0",
                    "rustc": "1.56.0",
                    "status": "incompatible"
                }
            ],
            "minimum_supported_rust_version": "1.40.0"
        }"#;

        let report: MsrvReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        let data = receipt.findings[0].data.as_ref().unwrap();
        assert_eq!(data.get("name").unwrap(), "test-crate");
        assert_eq!(data.get("version").unwrap(), "1.0.0");
        assert_eq!(data.get("msrv").unwrap(), "1.56.0");
        assert_eq!(data.get("rustc").unwrap(), "1.56.0");
        assert_eq!(data.get("status").unwrap(), "incompatible");
    }

    #[test]
    fn test_adapter_loads_from_file() {
        let adapter = CargoMsrvAdapter::new();
        let receipt = adapter
            .load(Path::new("tests/fixtures/report.json"))
            .expect("should load fixture");

        assert_eq!(adapter.sensor_id(), "cargo-msrv");
        assert!(!receipt.findings.is_empty());
    }

    #[test]
    fn test_adapter_returns_error_for_missing_file() {
        let adapter = CargoMsrvAdapter::new();
        let result = adapter.load(Path::new("nonexistent/path.json"));

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AdapterError::Io(_)));
    }

    #[test]
    fn test_adapter_returns_error_for_invalid_json() {
        let adapter = CargoMsrvAdapter::new();

        let temp_dir = tempfile::tempdir().unwrap();
        let temp_path = temp_dir.path().join("invalid.json");
        std::fs::write(&temp_path, "{ invalid json }").unwrap();

        let result = adapter.load(&temp_path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AdapterError::Json(_)));
    }

    #[test]
    fn test_multiple_findings() {
        let json = r#"{
            "results": [
                {
                    "name": "crate-a",
                    "version": "1.0.0",
                    "msrv": "1.56.0",
                    "rustc": "1.56.0",
                    "status": "incompatible"
                },
                {
                    "name": "crate-b",
                    "version": "2.0.0",
                    "msrv": "1.60.0",
                    "rustc": "1.60.0",
                    "status": "incompatible"
                },
                {
                    "name": "crate-c",
                    "version": "1.0.0",
                    "msrv": "1.50.0",
                    "rustc": "1.50.0",
                    "status": "compatible"
                }
            ],
            "minimum_supported_rust_version": "1.40.0"
        }"#;

        let report: MsrvReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 2);
        assert_eq!(receipt.verdict.counts.warnings, 2);
        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
    }
}
