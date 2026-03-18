use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub struct CargoMacheteAdapter;

impl CargoMacheteAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CargoMacheteAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for CargoMacheteAdapter {
    fn sensor_id(&self) -> &str {
        "cargo-machete"
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        let report: MacheteReport = serde_json::from_str(&content).map_err(AdapterError::Json)?;
        convert_report(report)
    }
}

fn convert_report(report: MacheteReport) -> Result<ReceiptEnvelope, AdapterError> {
    let mut findings = Vec::new();
    let mut warn_count = 0u64;

    if let Some(crates) = &report.crates {
        for machete_crate in crates {
            let check_id = "machete.unused_dependency";

            let location = Location {
                path: Utf8PathBuf::from(&machete_crate.manifest_path),
                line: None,
                column: None,
            };

            let message = format!(
                "unused dependency: {} (kind: {})",
                machete_crate.name, machete_crate.kind
            );

            let data = MacheteCrateData {
                name: machete_crate.name.clone(),
                kind: machete_crate.kind.clone(),
            };

            findings.push(Finding {
                severity: Severity::Warn,
                check_id: Some(check_id.to_string()),
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

    let mut builder = ReceiptBuilder::new("cargo-machete")
        .with_schema("cargo-machete.report.v1")
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
struct MacheteReport {
    crates: Option<Vec<MacheteCrate>>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
struct MacheteCrate {
    name: String,
    manifest_path: String,
    kind: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MacheteCrateData {
    name: String,
    kind: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = CargoMacheteAdapter::new();
        assert_eq!(adapter.sensor_id(), "cargo-machete");
    }

    #[test]
    fn test_convert_report_with_unused_crates() {
        let json = r#"{
            "crates": [
                {
                    "name": "unused-crate",
                    "manifest_path": "/path/to/Cargo.toml",
                    "kind": "direct"
                },
                {
                    "name": "transitive-crate",
                    "manifest_path": "/path/to/Cargo.toml",
                    "kind": "transitive"
                }
            ]
        }"#;

        let report: MacheteReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 2);

        let finding1 = &receipt.findings[0];
        assert_eq!(finding1.severity, Severity::Warn);
        assert_eq!(
            finding1.check_id,
            Some("machete.unused_dependency".to_string())
        );

        let finding2 = &receipt.findings[1];
        assert_eq!(finding2.severity, Severity::Warn);
        assert_eq!(
            finding2.check_id,
            Some("machete.unused_dependency".to_string())
        );

        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
        assert_eq!(receipt.verdict.counts.findings, 2);
        assert_eq!(receipt.verdict.counts.warnings, 2);
    }

    #[test]
    fn test_convert_report_empty_passes() {
        let json = r#"{
            "crates": []
        }"#;

        let report: MacheteReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_report_no_crates_passes() {
        let json = r#"{}"#;

        let report: MacheteReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_report_direct_kind() {
        let json = r#"{
            "crates": [
                {
                    "name": "some-crate",
                    "manifest_path": "/path/to/Cargo.toml",
                    "kind": "direct"
                }
            ]
        }"#;

        let report: MacheteReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert_eq!(
            receipt.findings[0].check_id,
            Some("machete.unused_dependency".to_string())
        );
    }

    #[test]
    fn test_convert_report_transitive_kind() {
        let json = r#"{
            "crates": [
                {
                    "name": "some-transitive",
                    "manifest_path": "/path/to/Cargo.toml",
                    "kind": "transitive"
                }
            ]
        }"#;

        let report: MacheteReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert_eq!(
            receipt.findings[0].check_id,
            Some("machete.unused_dependency".to_string())
        );
    }

    #[test]
    fn test_adapter_loads_from_file() {
        let adapter = CargoMacheteAdapter::new();
        let receipt = adapter
            .load(std::path::Path::new("tests/fixtures/report.json"))
            .expect("should load fixture");

        assert_eq!(adapter.sensor_id(), "cargo-machete");
        assert!(!receipt.findings.is_empty());
    }

    #[test]
    fn test_adapter_returns_error_for_missing_file() {
        let adapter = CargoMacheteAdapter::new();
        let result = adapter.load(std::path::Path::new("nonexistent/path.json"));

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AdapterError::Io(_)));
    }

    #[test]
    fn test_adapter_returns_error_for_invalid_json() {
        let adapter = CargoMacheteAdapter::new();

        // Create a temp file with invalid JSON
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
                    "manifest_path": "crates/test/Cargo.toml",
                    "kind": "direct"
                }
            ]
        }"#;

        let report: MacheteReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        let msg = receipt.findings[0].message.as_ref().unwrap();
        assert!(msg.contains("test-crate"));
        assert!(msg.contains("direct"));
    }
}
