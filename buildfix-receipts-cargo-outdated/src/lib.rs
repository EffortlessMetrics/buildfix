use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, AdapterMetadata, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub struct CargoOutdatedAdapter;

impl CargoOutdatedAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CargoOutdatedAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for CargoOutdatedAdapter {
    fn sensor_id(&self) -> &str {
        "cargo-outdated"
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        let report: OutdatedReport = serde_json::from_str(&content).map_err(AdapterError::Json)?;
        convert_report(report)
    }
}

impl AdapterMetadata for CargoOutdatedAdapter {
    fn name(&self) -> &str {
        "cargo-outdated"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn supported_schemas(&self) -> &[&str] {
        &["cargo-outdated.report.v1"]
    }
}

fn convert_report(report: OutdatedReport) -> Result<ReceiptEnvelope, AdapterError> {
    let mut findings = Vec::new();
    let mut warn_count = 0u64;

    if let Some(dependencies) = &report.dependencies {
        for dep in dependencies {
            let check_id = if dep.kind.as_deref() == Some("Dev") {
                "outdated.outdated"
            } else {
                "deps.outdated_dependency"
            };

            let location = Location {
                path: Utf8PathBuf::from("Cargo.toml"),
                line: None,
                column: None,
            };

            let message = format!(
                "{} v{} is outdated (latest: {})",
                dep.name, dep.version, dep.latest
            );

            let data = OutdatedDepData {
                name: dep.name.clone(),
                version: dep.version.clone(),
                latest: dep.latest.clone(),
                kind: dep.kind.clone().unwrap_or_default(),
                registry: dep.registry.clone(),
            };

            findings.push(Finding {
                severity: Severity::Warn,
                check_id: Some(check_id.to_string()),
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

    let mut builder = ReceiptBuilder::new("cargo-outdated")
        .with_schema("cargo-outdated.report.v1")
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
struct OutdatedReport {
    dependencies: Option<Vec<OutdatedDependency>>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
struct OutdatedDependency {
    name: String,
    version: String,
    latest: String,
    kind: Option<String>,
    #[serde(default)]
    registry: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OutdatedDepData {
    name: String,
    version: String,
    latest: String,
    kind: String,
    registry: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = CargoOutdatedAdapter::new();
        assert_eq!(adapter.sensor_id(), "cargo-outdated");
    }

    #[test]
    fn test_convert_report_with_outdated_deps() {
        let json = r#"{
            "dependencies": [
                {
                    "name": "serde",
                    "version": "1.0.0",
                    "latest": "1.0.200",
                    "kind": "Prod",
                    "project": "1.0.0",
                    "registry": "crates-io"
                },
                {
                    "name": "tokio",
                    "version": "1.0.0",
                    "latest": "1.40.0",
                    "kind": "Dev",
                    "project": "1.0.0",
                    "registry": "crates-io"
                }
            ]
        }"#;

        let report: OutdatedReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 2);

        let finding1 = &receipt.findings[0];
        assert_eq!(finding1.severity, Severity::Warn);
        assert_eq!(
            finding1.check_id,
            Some("deps.outdated_dependency".to_string())
        );
        assert!(finding1.message.as_ref().unwrap().contains("serde"));
        assert!(finding1.message.as_ref().unwrap().contains("1.0.0"));
        assert!(finding1.message.as_ref().unwrap().contains("1.0.200"));

        let finding2 = &receipt.findings[1];
        assert_eq!(finding2.severity, Severity::Warn);
        assert_eq!(finding2.check_id, Some("outdated.outdated".to_string()));

        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
        assert_eq!(receipt.verdict.counts.findings, 2);
        assert_eq!(receipt.verdict.counts.warnings, 2);
    }

    #[test]
    fn test_convert_report_empty_passes() {
        let json = r#"{
            "dependencies": []
        }"#;

        let report: OutdatedReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_report_no_dependencies_passes() {
        let json = r#"{}"#;

        let report: OutdatedReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_report_prod_kind_uses_deps_check_id() {
        let json = r#"{
            "dependencies": [
                {
                    "name": "some-crate",
                    "version": "1.0.0",
                    "latest": "2.0.0",
                    "kind": "Prod",
                    "registry": "crates-io"
                }
            ]
        }"#;

        let report: OutdatedReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert_eq!(
            receipt.findings[0].check_id,
            Some("deps.outdated_dependency".to_string())
        );
    }

    #[test]
    fn test_convert_report_dev_kind_uses_outdated_check_id() {
        let json = r#"{
            "dependencies": [
                {
                    "name": "some-crate",
                    "version": "1.0.0",
                    "latest": "2.0.0",
                    "kind": "Dev",
                    "registry": "crates-io"
                }
            ]
        }"#;

        let report: OutdatedReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert_eq!(
            receipt.findings[0].check_id,
            Some("outdated.outdated".to_string())
        );
    }

    #[test]
    fn test_convert_report_missing_kind_defaults_to_deps_check_id() {
        let json = r#"{
            "dependencies": [
                {
                    "name": "some-crate",
                    "version": "1.0.0",
                    "latest": "2.0.0",
                    "registry": "crates-io"
                }
            ]
        }"#;

        let report: OutdatedReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert_eq!(
            receipt.findings[0].check_id,
            Some("deps.outdated_dependency".to_string())
        );
    }

    #[test]
    fn test_adapter_loads_from_file() {
        let adapter = CargoOutdatedAdapter::new();
        let receipt = adapter
            .load(Path::new("tests/fixtures/report.json"))
            .expect("should load fixture");

        assert_eq!(adapter.sensor_id(), "cargo-outdated");
        assert!(!receipt.findings.is_empty());
    }

    #[test]
    fn test_adapter_returns_error_for_missing_file() {
        let adapter = CargoOutdatedAdapter::new();
        let result = adapter.load(Path::new("nonexistent/path.json"));

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AdapterError::Io(_)));
    }

    #[test]
    fn test_adapter_returns_error_for_invalid_json() {
        let adapter = CargoOutdatedAdapter::new();

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
            "dependencies": [
                {
                    "name": "test-crate",
                    "version": "1.2.3",
                    "latest": "4.5.6",
                    "kind": "Prod",
                    "registry": "crates-io"
                }
            ]
        }"#;

        let report: OutdatedReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        let msg = receipt.findings[0].message.as_ref().unwrap();
        assert!(msg.contains("test-crate"));
        assert!(msg.contains("v1.2.3"));
        assert!(msg.contains("4.5.6"));
    }

    #[test]
    fn test_finding_location_is_cargo_toml() {
        let json = r#"{
            "dependencies": [
                {
                    "name": "test-crate",
                    "version": "1.0.0",
                    "latest": "2.0.0",
                    "kind": "Prod",
                    "registry": "crates-io"
                }
            ]
        }"#;

        let report: OutdatedReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        let location = receipt.findings[0].location.as_ref().unwrap();
        assert_eq!(location.path.as_str(), "Cargo.toml");
    }

    #[test]
    fn test_finding_data_contains_dep_info() {
        let json = r#"{
            "dependencies": [
                {
                    "name": "test-crate",
                    "version": "1.0.0",
                    "latest": "2.0.0",
                    "kind": "Prod",
                    "registry": "crates-io"
                }
            ]
        }"#;

        let report: OutdatedReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        let data = receipt.findings[0].data.as_ref().unwrap();
        assert_eq!(data.get("name").unwrap(), "test-crate");
        assert_eq!(data.get("version").unwrap(), "1.0.0");
        assert_eq!(data.get("latest").unwrap(), "2.0.0");
        assert_eq!(data.get("kind").unwrap(), "Prod");
        assert_eq!(data.get("registry").unwrap(), "crates-io");
    }
}
