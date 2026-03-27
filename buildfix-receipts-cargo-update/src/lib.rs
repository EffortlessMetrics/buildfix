use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, AdapterMetadata, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub struct CargoUpdateAdapter;

impl CargoUpdateAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CargoUpdateAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for CargoUpdateAdapter {
    fn sensor_id(&self) -> &str {
        "cargo-update"
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        let report: UpdateReport = serde_json::from_str(&content).map_err(AdapterError::Json)?;
        convert_report(report)
    }
}

impl AdapterMetadata for CargoUpdateAdapter {
    fn name(&self) -> &str {
        "cargo-update"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn supported_schemas(&self) -> &[&str] {
        &["cargo-update.report.v1"]
    }
}

fn convert_report(report: UpdateReport) -> Result<ReceiptEnvelope, AdapterError> {
    let mut findings = Vec::new();
    let mut warn_count = 0u64;

    if let Some(crates) = &report.crates {
        for cr in crates {
            let location = Location {
                path: Utf8PathBuf::from("Cargo.toml"),
                line: None,
                column: None,
            };

            let message = format!(
                "{} v{} has an update available (latest: {}, stable: {})",
                cr.name, cr.version, cr.latest_version, cr.latest_stable
            );

            let data = UpdateCrateData {
                name: cr.name.clone(),
                version: cr.version.clone(),
                latest_version: cr.latest_version.clone(),
                latest_stable: cr.latest_stable.clone(),
            };

            findings.push(Finding {
                severity: Severity::Warn,
                check_id: Some("update.available".to_string()),
                code: None,
                message: Some(message),
                location: Some(location),
                fingerprint: None,
                data: Some(serde_json::to_value(data).unwrap_or_default()),
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

    let mut builder = ReceiptBuilder::new("cargo-update")
        .with_schema("cargo-update.report.v1")
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
struct UpdateReport {
    crates: Option<Vec<UpdateCrate>>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
struct UpdateCrate {
    name: String,
    version: String,
    #[serde(rename = "latest_version")]
    latest_version: String,
    #[serde(rename = "latest_stable")]
    latest_stable: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateCrateData {
    name: String,
    version: String,
    latest_version: String,
    latest_stable: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = CargoUpdateAdapter::new();
        assert_eq!(adapter.sensor_id(), "cargo-update");
    }

    #[test]
    fn test_convert_report_with_updates() {
        let json = r#"{
            "crates": [
                {
                    "name": "serde",
                    "version": "1.0.0",
                    "latest_version": "1.0.200",
                    "latest_stable": "1.0.200"
                },
                {
                    "name": "tokio",
                    "version": "1.0.0",
                    "latest_version": "1.40.0",
                    "latest_stable": "1.40.0"
                }
            ]
        }"#;

        let report: UpdateReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 2);

        let finding1 = &receipt.findings[0];
        assert_eq!(finding1.severity, Severity::Warn);
        assert_eq!(finding1.check_id, Some("update.available".to_string()));
        assert!(finding1.message.as_ref().unwrap().contains("serde"));
        assert!(finding1.message.as_ref().unwrap().contains("1.0.0"));
        assert!(finding1.message.as_ref().unwrap().contains("1.0.200"));

        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
        assert_eq!(receipt.verdict.counts.findings, 2);
        assert_eq!(receipt.verdict.counts.warnings, 2);
    }

    #[test]
    fn test_convert_report_empty_passes() {
        let json = r#"{
            "crates": []
        }"#;

        let report: UpdateReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_report_no_crates_passes() {
        let json = r#"{}"#;

        let report: UpdateReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_adapter_loads_from_file() {
        let adapter = CargoUpdateAdapter::new();
        let receipt = adapter
            .load(Path::new("tests/fixtures/report.json"))
            .expect("should load fixture");

        assert_eq!(adapter.sensor_id(), "cargo-update");
        assert!(!receipt.findings.is_empty());
    }

    #[test]
    fn test_adapter_returns_error_for_missing_file() {
        let adapter = CargoUpdateAdapter::new();
        let result = adapter.load(Path::new("nonexistent/path.json"));

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AdapterError::Io(_)));
    }

    #[test]
    fn test_adapter_returns_error_for_invalid_json() {
        let adapter = CargoUpdateAdapter::new();

        let temp_dir = tempfile::tempdir().unwrap();
        let temp_path = temp_dir.path().join("invalid.json");
        std::fs::write(&temp_path, "{ invalid json }").unwrap();

        let result = adapter.load(&temp_path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AdapterError::Json(_)));
    }

    #[test]
    fn test_finding_message_format() {
        let json = r#"{
            "crates": [
                {
                    "name": "test-crate",
                    "version": "1.2.3",
                    "latest_version": "4.5.6",
                    "latest_stable": "4.5.6"
                }
            ]
        }"#;

        let report: UpdateReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        let msg = receipt.findings[0].message.as_ref().unwrap();
        assert!(msg.contains("test-crate"));
        assert!(msg.contains("v1.2.3"));
        assert!(msg.contains("4.5.6"));
    }

    #[test]
    fn test_finding_location_is_cargo_toml() {
        let json = r#"{
            "crates": [
                {
                    "name": "test-crate",
                    "version": "1.0.0",
                    "latest_version": "2.0.0",
                    "latest_stable": "2.0.0"
                }
            ]
        }"#;

        let report: UpdateReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        let location = receipt.findings[0].location.as_ref().unwrap();
        assert_eq!(location.path.as_str(), "Cargo.toml");
    }

    #[test]
    fn test_finding_data_contains_crate_info() {
        let json = r#"{
            "crates": [
                {
                    "name": "test-crate",
                    "version": "1.0.0",
                    "latest_version": "2.0.0",
                    "latest_stable": "2.0.0"
                }
            ]
        }"#;

        let report: UpdateReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        let data = receipt.findings[0].data.as_ref().unwrap();
        assert_eq!(data.get("name").unwrap(), "test-crate");
        assert_eq!(data.get("version").unwrap(), "1.0.0");
        assert_eq!(data.get("latestVersion").unwrap(), "2.0.0");
        assert_eq!(data.get("latestStable").unwrap(), "2.0.0");
    }
}
